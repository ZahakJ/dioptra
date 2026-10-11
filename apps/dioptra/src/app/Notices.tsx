import { useNotices } from '../store/notices';
import { CloseIcon } from './icons';

export function Notices() {
  const notices = useNotices((s) => s.notices);
  const dismiss = useNotices((s) => s.dismiss);
  return (
    <div className="notices" role="status" aria-live="polite">
      {notices.map((n) => (
        <div key={n.id} className={`notice notice-${n.kind} popover`} data-testid={`notice-${n.kind}`}>
          <div className="notice-main">
            {n.kind === 'error' && <span className="notice-tag">Error</span>}
            <span className="notice-text">{n.text}</span>
            {n.action && (
              <button
                type="button"
                className="btn btn-small"
                data-testid="notice-action"
                onClick={() => {
                  dismiss(n.id);
                  void n.action!.run();
                }}
              >
                {n.action.label}
              </button>
            )}
            <button type="button" className="icon-btn" aria-label="Dismiss" onClick={() => dismiss(n.id)}>
              <CloseIcon size={10} />
            </button>
          </div>
          {n.detail && n.detail.length > 0 && (
            <ul className="notice-detail">
              {n.detail.map((d, i) => (
                <li key={i}>{d}</li>
              ))}
            </ul>
          )}
        </div>
      ))}
    </div>
  );
}
