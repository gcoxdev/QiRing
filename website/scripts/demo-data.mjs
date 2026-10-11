// Invented, disposable data for marketing screenshots. Never read a real Ring.
export function installDemoBridge() {
  const stamp = '2026-09-15T10:00:00Z';
  const names = ['Juniper Mail', 'Northstar Notes', 'Cloud Orchard', 'Cedar Journal', 'Daylight Calendar', 'Train Tickets', 'Studio Wi-Fi', 'Home Router'];
  const items = names.map((title, i) => ({
    id: `00000000-0000-4000-8000-${String(i + 1).padStart(12, '0')}`,
    item_type: 'login', title, username: 'alex@example.com',
    folder: i < 4 ? 'Everyday' : 'Personal', tags: i < 4 ? ['everyday'] : ['personal'],
    icon_data_url: null, has_totp: false, updated_at: stamp,
  }));
  const policy = { length: 20, upper: { min: 1, max: 20 }, lower: { min: 1, max: 20 }, numbers: { min: 1, max: 20 }, symbols: { min: 1, max: 20 }, allowed_symbols: '!@#$%^&*()-_=+[]{};:,.?/', avoid_ambiguous: false };
  const profiles = [
    { id: '11111111-1111-4111-8111-111111111111', name: 'Strong 20', policy },
    { id: '22222222-2222-4222-8222-222222222222', name: 'Alphanumeric 20', policy: { ...policy, symbols: { min: 0, max: 0 }, avoid_ambiguous: true } },
  ];
  const settings = { auto_lock_minutes: 5, clipboard_clear_seconds: 30, lock_on_window_blur: false, lock_on_minimize: true, biometric_enabled: false, theme: 'dark', button_display: 'both', ring_sort_mode: 'custom', ring_category_order: [], ring_item_order: [], backup_preferences: { include_settings: true, automatic_enabled: false, directory: null, retention_count: 10 } };
  let callbackId = 0;
  window.__demoUnexpectedCommands = [];
  window.__TAURI_INTERNALS__ = {
    transformCallback() { return ++callbackId; },
    unregisterCallback() {},
    async invoke(command, args = {}) {
      switch (command) {
        case 'plugin:event|listen': return args.handler;
        case 'plugin:event|unlisten': return null;
        case 'get_bootstrap_theme': return 'dark';
        case 'vault_exists': return true;
        case 'unlock_vault_master': return { session: { token: 'fictional-demo-session', unlocked_at: stamp }, migrated_recovery: null };
        case 'get_security_status': return { ...settings, schema_version: 2, command_version: '8', biometric_available: false };
        case 'list_profiles': return structuredClone(profiles);
        case 'list_items': return structuredClone(items);
        case 'get_item': return { ...items.find(item => item.id === args.itemId), password: 'DEMO-only!NeverUse42', url: 'https://juniper.example', notes: 'Fictional account for the QiRing product tour.', security_questions: [], custom_fields: [], totp_secret: null, password_history: [] };
        case 'get_settings': return structuredClone(settings);
        case 'generate_password': return { value: 'DEMO-only!NeverUse42x' };
        case 'health_report': return { analyzed_items: 8, weak_count: 1, reused_count: 0, old_count: 1, issues: [
          { item_id: items[0].id, title: items[0].title, kind: 'old', detail: 'Password has not been changed in over 180 days.' },
          { item_id: items[6].id, title: items[6].title, kind: 'weak', detail: 'Password is short or lacks character variety.' },
        ] };
        case 'list_snapshots': return [];
        case 'touch_activity':
        case 'set_bootstrap_theme':
        case 'lock_vault': return null;
        default:
          window.__demoUnexpectedCommands.push(command);
          throw new Error(`Screenshot mock does not support: ${command}`);
      }
    },
  };
}
