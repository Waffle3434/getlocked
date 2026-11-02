import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

const variants: Record<Variant, string> = {
  primary: "bg-accent-600 hover:bg-accent-500 text-on-accent shadow-lg shadow-accent-600/20",
  secondary: "bg-ink-800 hover:bg-ink-700 text-ink-100 border border-ink-700",
  ghost: "hover:bg-ink-800 text-ink-300 hover:text-ink-100",
  danger: "bg-danger-tint/10 hover:bg-danger-tint/20 text-danger border border-danger-tint/30",
};

export function Button({
  variant = "secondary",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      {...props}
      className={`inline-flex shrink-0 items-center justify-center gap-2 rounded-lg px-4 py-2 text-sm font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${variants[variant]} ${className}`}
    />
  );
}

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`rounded-2xl border border-ink-800 bg-ink-900 p-5 ${className}`}>{children}</div>;
}

export function SectionTitle({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
  return (
    <div className="mb-3">
      <h3 className="text-sm font-semibold text-ink-100">{children}</h3>
      {hint && <p className="mt-0.5 text-xs text-ink-400">{hint}</p>}
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
}) {
  return (
    <div className="inline-flex rounded-lg border border-ink-700 bg-ink-850 p-1">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          onClick={() => onChange(o.value)}
          className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
            value === o.value ? "bg-ink-700 text-ink-100 shadow" : "text-ink-400 hover:text-ink-100"
          }`}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: ReactNode;
  hint?: ReactNode;
}) {
  return (
    <label className="flex cursor-pointer items-start justify-between gap-4">
      <span>
        <span className="block text-sm text-ink-100">{label}</span>
        {hint && <span className="mt-0.5 block text-xs text-ink-400">{hint}</span>}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative mt-0.5 h-6 w-11 shrink-0 rounded-full transition-colors ${checked ? "bg-accent-600" : "bg-ink-700"}`}
      >
        <span
          className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-all ${checked ? "left-[22px]" : "left-0.5"}`}
        />
      </button>
    </label>
  );
}

export function Chip({ children, onRemove }: { children: ReactNode; onRemove?: () => void }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-ink-700 bg-ink-800 py-1 pr-1.5 pl-3 text-xs text-ink-100">
      {children}
      {onRemove && (
        <button
          type="button"
          onClick={onRemove}
          className="grid h-4 w-4 place-items-center rounded-full text-ink-400 hover:bg-ink-600 hover:text-ink-100"
          aria-label="Remove"
        >
          ×
        </button>
      )}
    </span>
  );
}

export function NumberField({
  value,
  onChange,
  min = 0,
  max,
  suffix,
}: {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  suffix?: string;
}) {
  return (
    <label className="inline-flex items-center gap-2 text-sm text-ink-300">
      <input
        type="number"
        value={Number.isFinite(value) ? value : ""}
        min={min}
        max={max}
        onChange={(e) => {
          const n = Math.floor(Number(e.target.value));
          onChange(Number.isFinite(n) ? Math.max(min, max === undefined ? n : Math.min(max, n)) : min);
        }}
        className="w-20 rounded-lg border border-ink-700 bg-ink-850 px-3 py-1.5 text-ink-100 outline-none focus:border-accent-500"
      />
      {suffix}
    </label>
  );
}

export function Modal({
  title,
  children,
  onClose,
  wide,
}: {
  title: ReactNode;
  children: ReactNode;
  onClose?: () => void;
  wide?: boolean;
}) {
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-backdrop p-6 backdrop-blur-sm">
      <div
        className={`flex max-h-full w-full flex-col overflow-hidden rounded-2xl border border-ink-700 bg-ink-900 shadow-2xl ${wide ? "max-w-3xl" : "max-w-lg"}`}
      >
        <div className="flex items-center justify-between border-b border-ink-800 px-5 py-4">
          <h2 className="text-base font-semibold">{title}</h2>
          {onClose && (
            <button onClick={onClose} className="text-xl leading-none text-ink-400 hover:text-ink-100" aria-label="Close">
              ×
            </button>
          )}
        </div>
        <div className="overflow-y-auto p-5">{children}</div>
      </div>
    </div>
  );
}

export function ErrorText({ children }: { children: ReactNode }) {
  if (!children) return null;
  return <p className="rounded-lg border border-danger-tint/30 bg-danger-tint/10 px-3 py-2 text-sm text-danger">{children}</p>;
}
