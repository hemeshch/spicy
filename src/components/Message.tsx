import { FileChange } from './FileChange';
import './Message.css';

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  changes?: { filename: string; description: string }[];
  isLoading?: boolean;
}

interface MessageProps {
  message: ChatMessage;
}

export function Message({ message }: MessageProps) {
  if (message.isLoading) {
    return (
      <div className="message assistant">
        <div className="message-header">
          <div className="assistant-dots">
            <span className="dot" style={{ background: 'var(--rainbow-green)' }} />
            <span className="dot" style={{ background: 'var(--rainbow-yellow)' }} />
            <span className="dot" style={{ background: 'var(--rainbow-orange)' }} />
          </div>
          <span className="assistant-label">spicy</span>
        </div>
        <div className="message-bubble assistant-bubble">
          <span className="thinking">thinking...</span>
        </div>
      </div>
    );
  }

  if (message.role === 'user') {
    return (
      <div className="message user">
        <div className="message-bubble user-bubble">
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className="message assistant">
      <div className="message-header">
        <div className="assistant-dots">
          <span className="dot" style={{ background: 'var(--rainbow-red)' }} />
          <span className="dot" style={{ background: 'var(--rainbow-purple)' }} />
          <span className="dot" style={{ background: 'var(--rainbow-blue)' }} />
        </div>
        <span className="assistant-label">spicy</span>
      </div>
      <div className="message-bubble assistant-bubble">
        {message.content}
      </div>
      {message.changes && message.changes.length > 0 && (
        <div className="message-changes">
          {message.changes.map((change, i) => (
            <FileChange key={i} index={i} change={change} />
          ))}
        </div>
      )}
    </div>
  );
}
