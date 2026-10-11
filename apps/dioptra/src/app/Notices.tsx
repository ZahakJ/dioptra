import { useNotices } from '../store/notices';
import { Button } from '../ui';
import { CloseIcon } from './icons';

export function Notices() {
  const notices = useNotices((s) => s.notices);
  const dismiss = useNotices((s) => s.dismiss);
  return (
    <div className="notices" role="status" aria-live="polite">
      {notices.map((n) => (
        <div key={n.id} className={`notice notice-${n.kind}`} data-testid={`notice-${n.kind}`}>
          <div className="notice-main">
            {n.kind === 'error' && <span className="notice-tag">Error</span>}
            <span className="notice-text">{n.text}</span>
            {n.action && (
              <Button
                size="sm"
                testId="notice-action"
                onClick={() => {
                  dismiss(n.id);
                  void n.action!.run();
                }}
              >
                {n.action.label}
              </Button>
            )}
            <Button variant="quiet" size="sm" iconOnly="Dismiss" icon={<CloseIcon size={16} />} onClick={() => dismiss(n.id)} />
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
