import { useState } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { FileChange } from './FileChange';
import './Message.css';

export interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  thinking?: string;
  changes?: { filename: string; description: string }[];
  isLoading?: boolean;
  isStreaming?: boolean;
}

const RAINBOW = [
  'var(--rainbow-green)',
  'var(--rainbow-yellow)',
  'var(--rainbow-orange)',
  'var(--rainbow-red)',
  'var(--rainbow-purple)',
  'var(--rainbow-blue)',
];

interface MessageProps {
  message: ChatMessage;
}

function ThinkingBlock({ thinking, isStreaming }: { thinking: string; isStreaming?: boolean }) {
  const [expanded, setExpanded] = useState(false);

  if (!thinking) return null;

  return (
    <div className={`thinking-block ${expanded ? 'expanded' : ''}`}>
      <button
        className="thinking-toggle"
        onClick={() => setExpanded(!expanded)}
      >
        <span className={`thinking-chevron ${expanded ? 'open' : ''}`}>&#9654;</span>
        <span className="thinking-label">
          {isStreaming ? 'spicy is thinking...' : 'spicy thought for a moment'}
        </span>
        {isStreaming && <span className="thinking-pulse" />}
      </button>
      {expanded && (
        <div className="thinking-content markdown-content">
          <Markdown remarkPlugins={[remarkGfm]}>{thinking}</Markdown>
        </div>
      )}
    </div>
  );
}

export function Message({ message }: MessageProps) {
  // Still waiting for first token
  if (message.isLoading) {
    return (
      <div className="message assistant">
        <div className="message-header">
          <div className="assistant-dots">
            {RAINBOW.map((c, i) => (
              <span key={i} className="dot" style={{ background: c }} />
            ))}
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

  // Streaming: show thinking + partial text as they arrive
  const isStreaming = message.isStreaming;
  const hasThinking = !!(message.thinking && message.thinking.length > 0);
  const hasContent = message.content.length > 0;

  return (
    <div className="message assistant">
      <div className="message-header">
        <div className="assistant-dots">
          {RAINBOW.map((c, i) => (
            <span key={i} className="dot" style={{ background: c }} />
          ))}
        </div>
        <span className="assistant-label">spicy</span>
      </div>

      {hasThinking && (
        <ThinkingBlock
          thinking={message.thinking!}
          isStreaming={isStreaming && !hasContent}
        />
      )}

      {hasContent && (
        <div className="message-bubble assistant-bubble">
          <div className="markdown-content">
            <Markdown remarkPlugins={[remarkGfm]}>{message.content}</Markdown>
          </div>
          {isStreaming && <span className="streaming-cursor" />}
        </div>
      )}

      {!hasContent && !isStreaming && (
        <div className="message-bubble assistant-bubble">{'\u200B'}</div>
      )}

      {isStreaming && !hasContent && !hasThinking && (
        <div className="message-bubble assistant-bubble">
          <span className="thinking">thinking...</span>
        </div>
      )}

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
