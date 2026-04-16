import { useAppStore } from "@/store";

function AppSettingsBanner() {
  const settingsError = useAppStore((s) => s.settingsError);
  const settingsMessage = useAppStore((s) => s.settingsMessage);

  if (!settingsError && !settingsMessage) {
    return null;
  }

  return (
    <>
      {settingsError ? (
        <p className="form-error settings-banner">{settingsError}</p>
      ) : null}
      {settingsMessage ? (
        <p className="stack-form__note settings-banner settings-banner--success">
          {settingsMessage}
        </p>
      ) : null}
    </>
  );
}

export default AppSettingsBanner;
