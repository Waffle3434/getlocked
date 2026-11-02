import { useEffect, useState, type ReactNode } from "react";
import { Check, CheckCircle2, Copy, ExternalLink, FolderOpen, Loader2, RefreshCw } from "lucide-react";
import { api, errorText } from "../api";
import type { BrowserInfo, ExtensionClient } from "../types";
import { Button, ErrorText, Modal } from "./ui";

function Step({ n, title, children }: { n: number; title: ReactNode; children?: ReactNode }) {
  return (
    <li className="flex gap-3">
      <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-accent-600/20 text-xs font-semibold text-accent-300">
        {n}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-2 pt-0.5">
        <div className="text-sm">{title}</div>
        {children}
      </div>
    </li>
  );
}

/** `k7qp-x2rm-…`, with copy and (optionally) reset buttons. */
export function PairingCode({ code, onReset }: { code: string; onReset?: () => void }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    await navigator.clipboard.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };
  return (
    <div className="flex flex-wrap items-center gap-2">
      <code className="rounded-md border border-ink-700 bg-ink-850 px-2 py-1 text-sm tracking-wide select-text">
        {code.match(/.{1,4}/g)?.join("-")}
      </code>
      <Button className="px-3 py-1 text-xs" onClick={copy}>
        {copied ? <Check size={14} /> : <Copy size={14} />} {copied ? "Copied" : "Copy"}
      </Button>
      {onReset && (
        <Button className="px-3 py-1 text-xs" onClick={onReset}>
          <RefreshCw size={14} /> New code
        </Button>
      )}
    </div>
  );
}

/** Walks the user through loading the extension into one of their browsers. */
export function ExtensionSetup({
  extensions,
  pairingCode,
  onClose,
}: {
  extensions: ExtensionClient[];
  pairingCode: string;
  onClose: () => void;
}) {
  const [browsers, setBrowsers] = useState<BrowserInfo[]>([]);
  const [chosen, setChosen] = useState<BrowserInfo | null>(null);
  const [folder, setFolder] = useState("");
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    api
      .listBrowsers()
      .then((list) => {
        setBrowsers(list);
        setChosen(list.find((b) => b.installed) ?? list[0] ?? null);
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  const target = chosen?.target;
  useEffect(() => {
    if (!target) return;
    setFolder("");
    api.prepareExtension(target).then(setFolder).catch((e) => setError(errorText(e)));
  }, [target]);

  const run = (p: Promise<unknown>) => {
    setError("");
    p.catch((e) => setError(errorText(e)));
  };

  const copy = async () => {
    await navigator.clipboard.writeText(folder);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  // An extension we couldn't identify reports "chromium", so it counts for
  // any Chromium browser.
  const connected =
    !!chosen &&
    extensions.some((e) => e.browser === chosen.id || (chosen.target === "chromium" && e.browser === "chromium"));

  const folderBox = (
    <div className="flex flex-col gap-2 rounded-lg border border-ink-700 bg-ink-850 p-3">
      <code className="text-xs break-all text-ink-300 select-text">{folder || "Preparing…"}</code>
      <div className="flex gap-2">
        <Button className="px-3 py-1 text-xs" onClick={copy} disabled={!folder}>
          {copied ? <Check size={14} /> : <Copy size={14} />} {copied ? "Copied" : "Copy path"}
        </Button>
        <Button className="px-3 py-1 text-xs" onClick={() => target && run(api.revealExtension(target))} disabled={!folder}>
          <FolderOpen size={14} /> Show in Explorer
        </Button>
      </div>
      <p className="text-xs text-ink-400">Tip: paste the path into the folder picker's address bar.</p>
    </div>
  );

  const openButton = chosen && (
    <div className="flex flex-wrap items-center gap-2">
      <Button className="px-3 py-1 text-xs" variant="primary" onClick={() => run(api.openExtensionsPage(chosen.id))} disabled={!chosen.installed}>
        <ExternalLink size={14} /> Open {chosen.name}
      </Button>
      <span className="text-xs text-ink-400">
        or type <code className="select-text">{chosen.extensionsUrl}</code> in the address bar
      </span>
    </div>
  );

  return (
    <Modal title="Set up website blocking" onClose={onClose} wide>
      <div className="flex flex-col gap-5">
        <p className="text-sm text-ink-300">
          The GetLocked extension blocks sites during a session. It only talks to this app on your PC. Store versions are
          on the way; for now it takes four clicks.
        </p>

        <div className="flex flex-wrap gap-2">
          {browsers.map((b) => (
            <button
              key={b.id}
              onClick={() => setChosen(b)}
              className={`rounded-lg border px-3 py-1.5 text-sm transition-colors ${
                chosen?.id === b.id
                  ? "border-accent-500 bg-accent-600/15 text-accent-300"
                  : b.installed
                    ? "border-ink-700 hover:border-ink-600"
                    : "border-ink-800 text-ink-400"
              }`}
            >
              {b.name}
              {!b.installed && <span className="ml-1 text-xs">(not found)</span>}
            </button>
          ))}
        </div>

        {chosen && chosen.target === "chromium" && (
          <ol className="flex flex-col gap-4">
            <Step n={1} title="Open the extensions page">{openButton}</Step>
            <Step
              n={2}
              title={
                <>
                  Turn on <strong>Developer mode</strong>{" "}
                  <span className="text-ink-400">
                    ({chosen.id === "edge" ? "bottom of the left sidebar" : "top-right corner"})
                  </span>
                </>
              }
            />
            <Step
              n={3}
              title={
                <>
                  Click <strong>Load unpacked</strong> and choose this folder
                </>
              }
            >
              {folderBox}
            </Step>
            <Step
              n={4}
              title={
                <>
                  Optional: in the extension's <strong>Details</strong>, turn on{" "}
                  <strong>{chosen.id === "edge" ? "Allow in InPrivate" : "Allow in Incognito"}</strong>{" "}
                  <span className="text-ink-400">(Hard mode needs this)</span>
                </>
              }
            />
          </ol>
        )}

        {chosen && chosen.target === "firefox" && (
          <ol className="flex flex-col gap-4">
            <Step n={1} title="Open Firefox's add-on debugging page">{openButton}</Step>
            <Step
              n={2}
              title={
                <>
                  Click <strong>Load Temporary Add-on…</strong> and choose <code>manifest.json</code> in this folder
                </>
              }
            >
              {folderBox}
            </Step>
            <Step
              n={3}
              title={
                <>
                  Click the GetLocked toolbar icon and press <strong>Grant access</strong> if it asks
                </>
              }
            />
            <p className="text-xs text-ink-400">
              Firefox removes temporary add-ons when it restarts, so you'll repeat this until the Firefox Add-ons version is
              out.
            </p>
          </ol>
        )}

        <div className="flex flex-col gap-2 text-xs text-ink-400">
          Loaded from this folder, the extension pairs with GetLocked by itself. If its popup asks for a pairing code,
          paste this one:
          <PairingCode code={pairingCode} />
        </div>

        <ErrorText>{error}</ErrorText>

        <div className="flex items-center justify-between gap-3 border-t border-ink-800 pt-4">
          {connected ? (
            <span className="flex items-center gap-2 text-sm text-ok">
              <CheckCircle2 size={16} /> Connected. Website blocking is ready in {chosen?.name}.
            </span>
          ) : (
            <span className="flex items-center gap-2 text-sm text-ink-400">
              <Loader2 size={16} className="animate-spin" /> Waiting for the extension to connect…
            </span>
          )}
          <Button variant={connected ? "primary" : "secondary"} onClick={onClose}>
            {connected ? "Done" : "Close"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
