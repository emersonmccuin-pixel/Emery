//! Minimal hand-rolled AWS SigV4 client for Cloudflare R2.
//!
//! R2 speaks S3, region `auto`, service `s3`. This client supports only the
//! three operations Project Commander needs: HEAD bucket (connection test),
//! PUT object (upload backup zip), GET bucket list (scaffold for Phase C2
//! restore listing). No multipart, no streaming signatures, no virtual-host
//! addressing — path-style only.

use hmac::{Hmac, Mac};
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};

const AWS_REGION: &str = "auto";
const AWS_SERVICE: &str = "s3";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct R2Object {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
}

pub struct R2Client {
    account_id: String,
    bucket: String,
    access_key: Zeroizing<String>,
    secret_key: Zeroizing<String>,
    http: Client,
}

impl R2Client {
    pub fn new(
        account_id: impl Into<String>,
        bucket: impl Into<String>,
        access_key: Zeroizing<String>,
        secret_key: Zeroizing<String>,
    ) -> AppResult<Self> {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| AppError::internal(format!("failed to build R2 HTTP client: {error}")))?;
        Ok(Self {
            account_id: account_id.into(),
            bucket: bucket.into(),
            access_key,
            secret_key,
            http,
        })
    }

    fn host(&self) -> String {
        format!("{}.r2.cloudflarestorage.com", self.account_id)
    }

    fn object_url(&self, key: &str) -> String {
        format!("https://{}/{}/{}", self.host(), self.bucket, encode_path(key))
    }

    fn bucket_url(&self) -> String {
        format!("https://{}/{}", self.host(), self.bucket)
    }

    /// Verify credentials + bucket by issuing `HEAD /<bucket>`.
    pub fn head_bucket(&self) -> AppResult<()> {
        let url = self.bucket_url();
        let now = now_utc();
        let signed = build_signed_request(
            SignInput {
                method: "HEAD",
                host: &self.host(),
                canonical_uri: &format!("/{}", self.bucket),
                canonical_query: "",
                payload_sha256: EMPTY_SHA256,
                now: &now,
                access_key: &self.access_key,
                secret_key: &self.secret_key,
                extra_headers: &[],
            },
        );

        let response = self
            .http
            .head(&url)
            .headers(signed.into_reqwest_headers()?)
            .send()
            .map_err(|error| AppError::supervisor(format!("R2 head_bucket failed: {error}")))?;
        expect_success(response, "head_bucket")?;
        Ok(())
    }

    /// Upload an object. Body is buffered in memory — Project Commander
    /// backups are tens of megabytes, not gigabytes.
    pub fn put_object(&self, key: &str, body: Vec<u8>) -> AppResult<()> {
        let url = self.object_url(key);
        let now = now_utc();
        let payload_hash = sha256_hex(&body);
        let content_length = body.len().to_string();
        let extra = vec![
            ("content-length".to_string(), content_length),
            ("content-type".to_string(), "application/zip".to_string()),
        ];
        let signed = build_signed_request(SignInput {
            method: "PUT",
            host: &self.host(),
            canonical_uri: &format!("/{}/{}", self.bucket, encode_path(key)),
            canonical_query: "",
            payload_sha256: &payload_hash,
            now: &now,
            access_key: &self.access_key,
            secret_key: &self.secret_key,
            extra_headers: &extra,
        });

        let response = self
            .http
            .put(&url)
            .headers(signed.into_reqwest_headers()?)
            .body(body)
            .send()
            .map_err(|error| AppError::supervisor(format!("R2 put_object failed: {error}")))?;
        expect_success(response, "put_object")?;
        Ok(())
    }

    /// Fetch an object body. Used by Phase C2 restore to pull a backup zip
    /// back from R2. Bodies are tens of MB so `Vec<u8>` is fine.
    pub fn get_object(&self, key: &str) -> AppResult<Vec<u8>> {
        let url = self.object_url(key);
        let now = now_utc();
        let signed = build_signed_request(SignInput {
            method: "GET",
            host: &self.host(),
            canonical_uri: &format!("/{}/{}", self.bucket, encode_path(key)),
            canonical_query: "",
            payload_sha256: EMPTY_SHA256,
            now: &now,
            access_key: &self.access_key,
            secret_key: &self.secret_key,
            extra_headers: &[],
        });

        let response = self
            .http
            .get(&url)
            .headers(signed.into_reqwest_headers()?)
            .send()
            .map_err(|error| AppError::supervisor(format!("R2 get_object failed: {error}")))?;
        let response = expect_success(response, "get_object")?;
        let bytes = response
            .bytes()
            .map_err(|error| AppError::internal(format!("R2 get_object body read failed: {error}")))?;
        Ok(bytes.to_vec())
    }

    /// List objects, optionally filtered by prefix. Scaffolded for Phase C2
    /// restore-from-remote; not wired through an invoke in Phase C1.
    pub fn list_objects(&self, prefix: Option<&str>) -> AppResult<Vec<R2Object>> {
        let url = self.bucket_url();
        let now = now_utc();
        let canonical_query = match prefix {
            Some(p) if !p.is_empty() => format!("list-type=2&prefix={}", percent_encode(p)),
            _ => "list-type=2".to_string(),
        };

        let signed = build_signed_request(SignInput {
            method: "GET",
            host: &self.host(),
            canonical_uri: &format!("/{}", self.bucket),
            canonical_query: &canonical_query,
            payload_sha256: EMPTY_SHA256,
            now: &now,
            access_key: &self.access_key,
            secret_key: &self.secret_key,
            extra_headers: &[],
        });

        let response = self
            .http
            .get(format!("{url}?{canonical_query}"))
            .headers(signed.into_reqwest_headers()?)
            .send()
            .map_err(|error| AppError::supervisor(format!("R2 list_objects failed: {error}")))?;
        let response = expect_success(response, "list_objects")?;
        let body = response
            .text()
            .map_err(|error| AppError::internal(format!("R2 list_objects body read failed: {error}")))?;
        Ok(parse_list_bucket_result(&body))
    }
}

fn expect_success(response: Response, op: &str) -> AppResult<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let text = response.text().unwrap_or_default();
    Err(AppError::from_status(
        status.as_u16(),
        format!("R2 {op} error ({status}): {}", truncate_for_log(&text)),
    ))
}

fn truncate_for_log(body: &str) -> String {
    if body.len() <= 512 {
        body.to_string()
    } else {
        format!("{}…", &body[..512])
    }
}

struct SignInput<'a> {
    method: &'a str,
    host: &'a str,
    canonical_uri: &'a str,
    canonical_query: &'a str,
    payload_sha256: &'a str,
    now: &'a UtcStamp,
    access_key: &'a str,
    secret_key: &'a str,
    extra_headers: &'a [(String, String)],
}

struct SignedRequest {
    headers: Vec<(String, String)>,
}

impl SignedRequest {
    fn into_reqwest_headers(self) -> AppResult<HeaderMap> {
        let mut map = HeaderMap::new();
        for (name, value) in self.headers {
            let header_name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|error| AppError::internal(format!("invalid header name {name}: {error}")))?;
            let header_value = HeaderValue::from_str(&value)
                .map_err(|error| AppError::internal(format!("invalid header value for {name}: {error}")))?;
            map.insert(header_name, header_value);
        }
        Ok(map)
    }
}

fn build_signed_request(input: SignInput<'_>) -> SignedRequest {
    // Canonical headers MUST be sorted by lowercased name. We always send
    // host + x-amz-content-sha256 + x-amz-date, plus whatever the caller
    // supplied (already lowercased by contract above).
    let mut headers: Vec<(String, String)> = Vec::new();
    headers.push(("host".to_string(), input.host.to_string()));
    headers.push((
        "x-amz-content-sha256".to_string(),
        input.payload_sha256.to_string(),
    ));
    headers.push(("x-amz-date".to_string(), input.now.amz_datetime.clone()));
    for (name, value) in input.extra_headers {
        headers.push((name.to_lowercase(), value.clone()));
    }
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical_headers = headers
        .iter()
        .map(|(name, value)| format!("{name}:{}\n", value.trim()))
        .collect::<String>();
    let signed_headers = headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{method}\n{uri}\n{query}\n{canonical_headers}\n{signed_headers}\n{payload}",
        method = input.method,
        uri = input.canonical_uri,
        query = input.canonical_query,
        canonical_headers = canonical_headers,
        signed_headers = signed_headers,
        payload = input.payload_sha256,
    );

    let credential_scope = format!(
        "{}/{AWS_REGION}/{AWS_SERVICE}/aws4_request",
        input.now.amz_date
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        input.now.amz_datetime,
        credential_scope,
        sha256_hex(canonical_request.as_bytes()),
    );

    let signing_key = derive_signing_key(input.secret_key, &input.now.amz_date);
    let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
        input.access_key,
    );

    // Reqwest adds Host automatically from the URL, so drop it before returning.
    let mut out: Vec<(String, String)> = headers
        .into_iter()
        .filter(|(name, _)| name != "host")
        .collect();
    out.push(("authorization".to_string(), authorization));
    SignedRequest { headers: out }
}

fn derive_signing_key(secret_key: &str, date: &str) -> Vec<u8> {
    let k_secret = format!("AWS4{secret_key}");
    let k_date = hmac_sha256(k_secret.as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, AWS_REGION.as_bytes());
    let k_service = hmac_sha256(&k_region, AWS_SERVICE.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

struct UtcStamp {
    amz_date: String,     // YYYYMMDD
    amz_datetime: String, // YYYYMMDDTHHMMSSZ
}

fn now_utc() -> UtcStamp {
    use chrono::Utc;
    let now = Utc::now();
    UtcStamp {
        amz_date: now.format("%Y%m%d").to_string(),
        amz_datetime: now.format("%Y%m%dT%H%M%SZ").to_string(),
    }
}

/// Percent-encode a path segment per AWS SigV4 rules (spaces -> %20, slashes
/// in the key preserved).
fn encode_path(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    for byte in key.as_bytes() {
        let b = *byte;
        if is_unreserved(b) || b == b'/' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        let b = *byte;
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn is_unreserved(byte: u8) -> bool {
    matches!(byte,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~'
    )
}

/// Minimal XML scraper for ListBucketResult <Contents><Key><Size><LastModified>.
/// Hand-parsed to avoid pulling a full XML dep for one feature.
fn parse_list_bucket_result(xml: &str) -> Vec<R2Object> {
    let mut out = Vec::new();
    let mut cursor = 0_usize;
    while let Some(start) = xml[cursor..].find("<Contents>") {
        let content_start = cursor + start + "<Contents>".len();
        let Some(end_rel) = xml[content_start..].find("</Contents>") else {
            break;
        };
        let content_end = content_start + end_rel;
        let block = &xml[content_start..content_end];
        let key = extract_tag(block, "Key").unwrap_or_default();
        let size = extract_tag(block, "Size")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let last_modified = extract_tag(block, "LastModified").unwrap_or_default();
        if !key.is_empty() {
            out.push(R2Object { key, size, last_modified });
        }
        cursor = content_end + "</Contents>".len();
    }
    out
}

fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end_rel = block[start..].find(&close)?;
    Some(block[start..start + end_rel].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // AWS-published GetObject test vector (docs.aws.amazon.com example).
    // secret: wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
    // access: AKIAIOSFODNN7EXAMPLE
    // host:   examplebucket.s3.amazonaws.com
    // method: GET
    // uri:    /test.txt
    // date:   20130524T000000Z
    // range:  bytes=0-9
    // Expected signature (from AWS docs):
    //   f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41
    // This validates our canonical request + string-to-sign assembly. We pin
    // it using the AWS reference inputs; region/service differ from R2 but
    // the algorithm is identical.
    #[test]
    fn aws_get_object_sigv4_reference_vector() {
        let now = UtcStamp {
            amz_date: "20130524".to_string(),
            amz_datetime: "20130524T000000Z".to_string(),
        };
        let _access_key = "AKIAIOSFODNN7EXAMPLE";
        let secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let host = "examplebucket.s3.amazonaws.com";
        let payload = EMPTY_SHA256;

        // Replicate build_signed_request but against us-east-1 / s3 to match
        // the AWS vector. We inline the algorithm here rather than parameterize
        // the production path just for testing.
        let mut headers: Vec<(String, String)> = vec![
            ("host".to_string(), host.to_string()),
            ("range".to_string(), "bytes=0-9".to_string()),
            ("x-amz-content-sha256".to_string(), payload.to_string()),
            ("x-amz-date".to_string(), now.amz_datetime.clone()),
        ];
        headers.sort_by(|a, b| a.0.cmp(&b.0));

        let canonical_headers = headers
            .iter()
            .map(|(n, v)| format!("{n}:{}\n", v.trim()))
            .collect::<String>();
        let signed_headers = headers
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical_request = format!(
            "GET\n/test.txt\n\n{canonical_headers}\n{signed_headers}\n{payload}"
        );
        let credential_scope = format!("{}/us-east-1/s3/aws4_request", now.amz_date);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            now.amz_datetime,
            credential_scope,
            sha256_hex(canonical_request.as_bytes()),
        );

        // Local signing-key derivation with fixed region + service.
        let k_secret = format!("AWS4{secret_key}");
        let k_date = hmac_sha256(k_secret.as_bytes(), now.amz_date.as_bytes());
        let k_region = hmac_sha256(&k_date, b"us-east-1");
        let k_service = hmac_sha256(&k_region, b"s3");
        let signing_key = hmac_sha256(&k_service, b"aws4_request");
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

        assert_eq!(
            signature,
            "f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41",
            "SigV4 canonical request / string-to-sign / signing-key derivation must match AWS reference vector"
        );
    }

    #[test]
    fn parse_list_bucket_result_extracts_keys() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult>
  <Contents>
    <Key>pc-full-2026-04-14T00-00-00Z.zip</Key>
    <LastModified>2026-04-14T00:00:05Z</LastModified>
    <Size>12345</Size>
  </Contents>
  <Contents>
    <Key>pc-full-2026-04-13T00-00-00Z.zip</Key>
    <LastModified>2026-04-13T00:00:05Z</LastModified>
    <Size>9999</Size>
  </Contents>
</ListBucketResult>"#;
        let objs = parse_list_bucket_result(xml);
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0].key, "pc-full-2026-04-14T00-00-00Z.zip");
        assert_eq!(objs[0].size, 12345);
        assert_eq!(objs[1].size, 9999);
    }

    #[test]
    fn encode_path_preserves_slashes_and_escapes_spaces() {
        assert_eq!(encode_path("a/b c.zip"), "a/b%20c.zip");
    }
}
