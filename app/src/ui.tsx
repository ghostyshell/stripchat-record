// Shared UI primitives for the Stripchat Recorder shell.

import { ReactNode } from "react";

export function fmtBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / Math.pow(1024, i);
  return `${i === 0 ? bytes : value.toFixed(1)} ${units[i]}`;
}

export function fmtDate(unix: number): string {
  if (!unix) return "-";
  return new Date(unix * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function Chip({
  tone = "neutral",
  live = false,
  children,
  title,
}: {
  tone?: "neutral" | "good" | "warn" | "bad" | "laser";
  live?: boolean;
  children: ReactNode;
  title?: string;
}) {
  return (
    <span className={`chip ${tone} ${live ? "live" : ""}`} title={title}>
      <span className="dot" aria-hidden="true" />
      {children}
    </span>
  );
}

export function Btn({
  children,
  onClick,
  variant = "ghost",
  small = false,
  disabled,
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "ghost" | "primary" | "danger" | "outline";
  small?: boolean;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      className={`btn ${variant} ${small ? "small" : ""}`}
      onClick={onClick}
      disabled={disabled}
      title={title}
    >
      {children}
    </button>
  );
}

export function Seg<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  ariaLabel: string;
}) {
  return (
    <div className="seg" role="tablist" aria-label={ariaLabel}>
      {options.map((opt) => (
        <button
          key={opt.value}
          role="tab"
          aria-selected={value === opt.value}
          className={value === opt.value ? "on" : ""}
          onClick={() => onChange(opt.value)}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

export function Bar({ value, max, live = false }: { value: number; max: number; live?: boolean }) {
  const pct = max > 0 ? Math.min(100, Math.round((value / max) * 100)) : 0;
  return (
    <div
      className={`bar ${live ? "live" : ""}`}
      role="progressbar"
      aria-valuenow={pct}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className="fill" style={{ width: `${pct}%` }} />
    </div>
  );
}

export function Empty({ title, hint, action }: { title: string; hint?: string; action?: ReactNode }) {
  return (
    <div className="empty">
      <div className="empty-title">{title}</div>
      {hint ? <div className="empty-hint">{hint}</div> : null}
      {action ? <div className="empty-action">{action}</div> : null}
    </div>
  );
}

export function Eyebrow({ children }: { children: ReactNode }) {
  return <div className="eyebrow">{children}</div>;
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
}) {
  return (
    <button
      className={`switch ${checked ? "on" : ""}`}
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
    >
      <span className="knob" aria-hidden="true" />
      <span className="switch-label">{label}</span>
    </button>
  );
}

export function confirmDialog(title: string, body: string): boolean {
  // eslint-disable-next-line no-alert
  return window.confirm(`${title}\n\n${body}`);
}
