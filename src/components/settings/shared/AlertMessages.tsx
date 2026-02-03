interface AlertMessagesProps {
  error: string | null;
  success: string | null;
}

export function AlertMessages({ error, success }: AlertMessagesProps) {
  return (
    <>
      {error && (
        <div className="bg-status-error/10 border border-status-error/30 text-status-error px-3 py-2 rounded-lg text-sm">
          {error}
        </div>
      )}

      {success && (
        <div className="bg-status-success/10 border border-status-success/30 text-status-success px-3 py-2 rounded-lg text-sm">
          {success}
        </div>
      )}
    </>
  );
}
