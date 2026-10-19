export type ProxyType = 'socks5' | 'http';

export interface ProxyConfig {
  protocol: ProxyType;
  host: string;
  port: number;
  username?: string | null;
  password?: string | null;
}

export type MatchType = 'exact' | 'prefix' | 'wildcard';

export interface AppRule {
  id: string;
  name: string;
  path: string;
  match_type: MatchType;
  include_children: boolean;
  enabled: boolean;
}

export interface AppSettings {
  bypass_lan: boolean;
  block_udp_443: boolean;
  auto_start: boolean;
  minimize_to_tray: boolean;
}

export interface AppConfig {
  proxy: ProxyConfig;
  rules: AppRule[];
  settings: AppSettings;
}

export interface ProcessInfo {
  pid: number;
  name: string;
  path: string;
}

export interface ConnectionLog {
  id: string;
  timestamp: string;
  process_name: string;
  pid: number;
  target: string;
  protocol: string;
  status: 'success' | 'failed' | 'connecting';
  error_msg?: string | null;
}
