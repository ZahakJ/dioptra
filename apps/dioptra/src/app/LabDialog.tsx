// Help → Dioptra Lab: the license key, what it covers, and where to buy one.
import { useState } from "react";
import { PRICING_URL, standingText, useLab } from "../store/lab";
import { notify, notifyError } from "../store/notices";
import { Button, DialogFrame, DialogHeader, TextArea } from "../ui";
import { openExternal } from "./actions";

export function LabDialog() {
  const open = useLab((s) => s.dialogOpen);
  const setOpen = useLab((s) => s.setDialogOpen);
  const view = useLab((s) => s.view);
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  if (!open) return null;
  const close = () => {
    setOpen(false);
    setKey("");
    setError(null);
  };
  const activate = async () => {
    setBusy(true);
    try {
      const v = await useLab.getState().activate(key);
      if (v.error) setError(v.error);
      else {
        setKey("");
        setError(null);
        notify(`Dioptra Lab is on. Thank you, ${v.license?.licensee}.`);
      }
    } catch (err) {
      notifyError(err);
    } finally {
      setBusy(false);
    }
  };
  const l = view.license;
  return (
    <DialogFrame onCancel={close} label="Dioptra Lab" testId="lab-dialog">
      <div className="modal-body">
        <DialogHeader
          title="Dioptra Lab"
          sub="Batch analysis, instrument import, journal figures, Prism export and audit trails. Everything else in Dioptra is free."
          onClose={close}
        />
        {l ? (
          <div className="kv-list lab-license" data-testid="lab-license">
            <div className="kv">
              <span className="kv-label">Licensed to</span>
              <span className="kv-value">{l.licensee}</span>
            </div>
            <div className="kv">
              <span className="kv-label">Plan</span>
              <span className="kv-value">
                {view.plan_label}
                {l.plan === "site" && l.site
                  ? ` (${l.site})`
                  : l.seats > 1
                    ? ` (${l.seats} seats)`
                    : ""}
              </span>
            </div>
            <div className="kv">
              <span className="kv-label">Status</span>
              <span className="kv-value" data-testid="lab-standing">
                {standingText(view)}
              </span>
            </div>
            <div className="kv">
              <span className="kv-label">License</span>
              <span className="kv-value mono">{l.id}</span>
            </div>
          </div>
        ) : (
          <p className="muted lab-standing" data-testid="lab-standing">
            {standingText(view)}
          </p>
        )}
        <label className="lab-key-label" htmlFor="lab-key">
          {l ? "Replace the key" : "License key"}
        </label>
        <TextArea
          id="lab-key"
          mono
          rows={4}
          testId="lab-key"
          placeholder="DIOPTRA-LAB-1.…"
          value={key}
          onChange={(e) => {
            setKey(e.target.value);
            setError(null);
          }}
        />
        {error && (
          <p className="field-error" role="alert" data-testid="lab-error">
            {error}
          </p>
        )}
        <p className="muted lab-fine">
          The key is checked on this computer; Dioptra never contacts a server.
          When a license ends, Lab features stay on for 30 days and then pause.
          Your projects always open.
        </p>
        <footer className="modal-foot">
          {l && (
            <Button
              variant="quiet"
              className="lab-remove"
              onClick={() => void useLab.getState().remove()}
              data-testid="lab-remove"
            >
              Remove key
            </Button>
          )}
          <Button
            variant="secondary"
            onClick={() => void openExternal(PRICING_URL)}
          >
            {l ? "Renew" : "Buy a license"}
          </Button>
          <Button
            variant="primary"
            disabled={!key.trim() || busy}
            onClick={() => void activate()}
            data-testid="lab-activate"
          >
            Activate
          </Button>
        </footer>
      </div>
    </DialogFrame>
  );
}
