const platform = process.argv[2];

const required = {
  linux: ["LINUX_GPG_PRIVATE_KEY", "LINUX_GPG_KEY_ID", "LINUX_GPG_PASSPHRASE"],
  macos: ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "KEYCHAIN_PASSWORD"],
  windows: ["WINDOWS_CERTIFICATE", "WINDOWS_CERTIFICATE_PASSWORD", "WINDOWS_TIMESTAMP_URL"]
}[platform];

if (!required) throw new Error(`Unknown release platform: ${platform}`);
const missing = required.filter((name) => !process.env[name]);

if (platform === "macos") {
  const hasApiKey = ["APPLE_API_ISSUER", "APPLE_API_KEY", "APPLE_API_PRIVATE_KEY"]
    .every((name) => process.env[name]);
  const hasAppleId = ["APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"]
    .every((name) => process.env[name]);
  if (!hasApiKey && !hasAppleId) {
    missing.push("Apple notarization credentials (API key or Apple ID set)");
  }
}

if (missing.length > 0) {
  throw new Error(`Tagged ${platform} release is missing: ${missing.join(", ")}`);
}
