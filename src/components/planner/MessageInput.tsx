import { useState, useRef, useEffect } from 'react';

interface MessageInputProps {
  onSend: (message: string) => void;
  onCancel?: () => void;
  disabled?: boolean;
  isGenerating?: boolean;
  placeholder?: string;
}

export function MessageInput({
  onSend,
  onCancel,
  disabled,
  isGenerating,
  placeholder = 'Type a message...',
}: MessageInputProps) {
  const [value, setValue] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 300)}px`;
    }
  }, [value]);

  const handleSubmit = () => {
    if (value.trim() && !disabled) {
      onSend(value.trim());
      setValue('');
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="flex items-end gap-3">
      <div className="flex-1 relative">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder}
          disabled={disabled}
          rows={4}
          className="w-full px-3 py-2 glass rounded-xl resize-none text-xs text-board-text placeholder:text-board-text-muted focus:outline-none focus:ring-2 focus:ring-board-accent/50 disabled:opacity-50 disabled:cursor-not-allowed"
        />
      </div>
      {isGenerating && onCancel ? (
        <button
          onClick={onCancel}
          className="flex items-center justify-center w-12 h-12 rounded-xl bg-red-500/80 hover:bg-red-500 transition-all duration-200"
          title="Stop generation"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-4 w-4 text-white"
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <rect x="5" y="5" width="10" height="10" rx="1" />
          </svg>
        </button>
      ) : (
        <button
          onClick={handleSubmit}
          disabled={disabled || !value.trim()}
          className="flex items-center justify-center w-12 h-12 rounded-xl bg-board-accent hover:bg-board-accent-hover disabled:opacity-50 disabled:cursor-not-allowed transition-all duration-200"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="h-5 w-5 text-white"
            viewBox="0 0 20 20"
            fill="currentColor"
          >
            <path
              fillRule="evenodd"
              d="M10.293 3.293a1 1 0 011.414 0l6 6a1 1 0 010 1.414l-6 6a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-4.293-4.293a1 1 0 010-1.414z"
              clipRule="evenodd"
            />
          </svg>
        </button>
      )}
    </div>
  );
}
