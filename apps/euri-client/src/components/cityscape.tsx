/**
 * Cityscape — static SVG silhouette fixed to the bottom of the app shell.
 * Renders in the neon-forward themes and stays non-interactive.
 * pointer-events: none so it never intercepts clicks.
 */
export function Cityscape() {
  return (
    <svg
      className="cityscape-silhouette"
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 1440 110"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      {/* Buildings — varying widths and heights for a synthwave skyline */}
      {/* Far-left cluster */}
      <rect x="0" y="70" width="38" height="40" fill="rgba(5,3,12,0.88)" />
      <rect x="40" y="50" width="25" height="60" fill="rgba(5,3,12,0.88)" />
      {/* Antenna on second building */}
      <rect x="51" y="38" width="2" height="12" fill="rgba(5,3,12,0.88)" />
      <rect x="55" y="60" width="18" height="50" fill="rgba(5,3,12,0.88)" />
      <rect x="75" y="42" width="32" height="68" fill="rgba(5,3,12,0.88)" />
      {/* Antenna */}
      <rect x="89" y="30" width="3" height="12" fill="rgba(5,3,12,0.88)" />
      <rect x="109" y="65" width="22" height="45" fill="rgba(5,3,12,0.88)" />
      <rect x="133" y="55" width="40" height="55" fill="rgba(5,3,12,0.88)" />
      <rect x="175" y="72" width="20" height="38" fill="rgba(5,3,12,0.88)" />
      <rect x="197" y="35" width="28" height="75" fill="rgba(5,3,12,0.88)" />
      {/* Tall antenna tower */}
      <rect x="209" y="10" width="3" height="25" fill="rgba(5,3,12,0.88)" />
      <rect x="207" y="18" width="7" height="2" fill="rgba(5,3,12,0.88)" />
      {/* Mid-left cluster */}
      <rect x="227" y="60" width="30" height="50" fill="rgba(5,3,12,0.88)" />
      <rect x="259" y="45" width="22" height="65" fill="rgba(5,3,12,0.88)" />
      <rect x="283" y="75" width="35" height="35" fill="rgba(5,3,12,0.88)" />
      <rect x="320" y="48" width="42" height="62" fill="rgba(5,3,12,0.88)" />
      {/* Antenna */}
      <rect x="338" y="34" width="3" height="14" fill="rgba(5,3,12,0.88)" />
      <rect x="364" y="68" width="28" height="42" fill="rgba(5,3,12,0.88)" />
      <rect x="394" y="38" width="20" height="72" fill="rgba(5,3,12,0.88)" />
      <rect x="416" y="58" width="36" height="52" fill="rgba(5,3,12,0.88)" />
      {/* Center feature — tall tower with antenna */}
      <rect x="455" y="20" width="50" height="90" fill="rgba(5,3,12,0.88)" />
      <rect x="476" y="4" width="4" height="16" fill="rgba(5,3,12,0.88)" />
      <rect x="472" y="14" width="12" height="2" fill="rgba(5,3,12,0.88)" />
      {/* Center-right cluster */}
      <rect x="507" y="60" width="30" height="50" fill="rgba(5,3,12,0.88)" />
      <rect x="539" y="42" width="25" height="68" fill="rgba(5,3,12,0.88)" />
      <rect x="566" y="70" width="40" height="40" fill="rgba(5,3,12,0.88)" />
      <rect x="608" y="50" width="28" height="60" fill="rgba(5,3,12,0.88)" />
      <rect x="638" y="38" width="22" height="72" fill="rgba(5,3,12,0.88)" />
      {/* Antenna */}
      <rect x="647" y="24" width="3" height="14" fill="rgba(5,3,12,0.88)" />
      <rect x="662" y="62" width="34" height="48" fill="rgba(5,3,12,0.88)" />
      <rect x="698" y="45" width="38" height="65" fill="rgba(5,3,12,0.88)" />
      {/* Right-center cluster */}
      <rect x="738" y="72" width="24" height="38" fill="rgba(5,3,12,0.88)" />
      <rect x="764" y="55" width="30" height="55" fill="rgba(5,3,12,0.88)" />
      <rect x="796" y="35" width="45" height="75" fill="rgba(5,3,12,0.88)" />
      {/* Tall antenna tower */}
      <rect x="814" y="12" width="4" height="23" fill="rgba(5,3,12,0.88)" />
      <rect x="810" y="20" width="12" height="2" fill="rgba(5,3,12,0.88)" />
      <rect x="843" y="65" width="22" height="45" fill="rgba(5,3,12,0.88)" />
      <rect x="867" y="48" width="32" height="62" fill="rgba(5,3,12,0.88)" />
      <rect x="901" y="68" width="28" height="42" fill="rgba(5,3,12,0.88)" />
      <rect x="931" y="40" width="20" height="70" fill="rgba(5,3,12,0.88)" />
      {/* Antenna */}
      <rect x="939" y="26" width="3" height="14" fill="rgba(5,3,12,0.88)" />
      <rect x="953" y="58" width="38" height="52" fill="rgba(5,3,12,0.88)" />
      {/* Far-right cluster */}
      <rect x="993" y="72" width="30" height="38" fill="rgba(5,3,12,0.88)" />
      <rect x="1025" y="52" width="22" height="58" fill="rgba(5,3,12,0.88)" />
      <rect x="1049" y="38" width="40" height="72" fill="rgba(5,3,12,0.88)" />
      {/* Antenna */}
      <rect x="1065" y="24" width="3" height="14" fill="rgba(5,3,12,0.88)" />
      <rect x="1091" y="65" width="26" height="45" fill="rgba(5,3,12,0.88)" />
      <rect x="1119" y="44" width="36" height="66" fill="rgba(5,3,12,0.88)" />
      <rect x="1157" y="68" width="22" height="42" fill="rgba(5,3,12,0.88)" />
      <rect x="1181" y="55" width="30" height="55" fill="rgba(5,3,12,0.88)" />
      <rect x="1213" y="32" width="48" height="78" fill="rgba(5,3,12,0.88)" />
      {/* Antenna */}
      <rect x="1233" y="16" width="4" height="16" fill="rgba(5,3,12,0.88)" />
      <rect x="1229" y="24" width="12" height="2" fill="rgba(5,3,12,0.88)" />
      <rect x="1263" y="62" width="28" height="48" fill="rgba(5,3,12,0.88)" />
      <rect x="1293" y="48" width="20" height="62" fill="rgba(5,3,12,0.88)" />
      <rect x="1315" y="70" width="35" height="40" fill="rgba(5,3,12,0.88)" />
      <rect x="1352" y="54" width="25" height="56" fill="rgba(5,3,12,0.88)" />
      <rect x="1379" y="38" width="30" height="72" fill="rgba(5,3,12,0.88)" />
      <rect x="1411" y="65" width="29" height="45" fill="rgba(5,3,12,0.88)" />
      {/* Ground fill to avoid any gap at very bottom */}
      <rect x="0" y="108" width="1440" height="2" fill="rgba(5,3,12,0.88)" />
    </svg>
  );
}
