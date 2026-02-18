import { useState, useRef, useEffect } from 'react';
import { Message, type ChatMessage } from './Message';
import type { ChatSessionMeta } from '../hooks/useChat';
import './Chat.css';

const RAINBOW = ['#61BB46', '#FDB827', '#F5821F', '#E03A3E', '#963D97', '#009DDC'];
const SUGGESTIONS = ['modify components', 'add subcircuits', 'adjust params', 'debug convergence'];
const PILL_COLORS = [RAINBOW[0], RAINBOW[1], RAINBOW[3], RAINBOW[5]];

interface ChatProps {
  messages: ChatMessage[];
  isLoading: boolean;
  onSend: (message: string) => void;
  promptColor: string;
  activeFile: string | null;
  sessions: ChatSessionMeta[];
  activeSessionId: string | null;
  onSwitchSession: (id: string) => void;
  onNewSession: () => void;
}

export function Chat({
  messages,
  isLoading,
  onSend,
  promptColor,
  activeFile,
  sessions,
  activeSessionId,
  onSwitchSession,
  onNewSession,
}: ChatProps) {
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isLoading]);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = input.trim();
    if (!trimmed || isLoading) return;
    onSend(trimmed);
    setInput('');
  };

  const showHeader = sessions.length > 0 || messages.length > 0;
  const isEmptySession = messages.length === 0 && !activeSessionId;

  return (
    <div className="chat">
      {showHeader && (
        <div className="chat-header">
          <select
            className="session-select"
            value={activeSessionId || ''}
            onChange={(e) => {
              if (e.target.value) onSwitchSession(e.target.value);
            }}
            disabled={isLoading}
          >
            {!activeSessionId && (
              <option value="">new chat</option>
            )}
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {s.title} ({s.message_count})
              </option>
            ))}
          </select>
          <button
            className="new-chat-btn"
            onClick={onNewSession}
            disabled={isLoading || isEmptySession}
            title="Start new chat"
          >
            + new
          </button>
        </div>
      )}
      <div className="chat-messages">
        {messages.length === 0 && !isLoading ? (
          <div className="chat-empty">
            <div className="chat-empty-dots">
              {RAINBOW.map((c, i) => (
                <span key={i} className="dot" style={{ background: c }} />
              ))}
            </div>
            <h2 className="chat-empty-title">spicy</h2>
            <p className="chat-empty-subtitle">
              describe what you want to change. i'll edit your .asc files directly.
            </p>
            <div className="chat-empty-pills">
              {SUGGESTIONS.map((label, i) => (
                <span key={label} className="chat-empty-pill" style={{ borderColor: PILL_COLORS[i], color: PILL_COLORS[i], background: `${PILL_COLORS[i]}12` }}>
                  {label}
                </span>
              ))}
            </div>
          </div>
        ) : (
          <>
            {messages.map((msg) => (
              <Message key={msg.id} message={msg} />
            ))}
            {isLoading && !messages.some((m) => m.isStreaming) && (
              <Message
                message={{
                  id: 'loading',
                  role: 'assistant',
                  content: '',
                  isLoading: true,
                }}
              />
            )}
          </>
        )}
        <div ref={messagesEndRef} />
      </div>
      <form className="chat-input-bar" onSubmit={handleSubmit}>
        <div className="input-container">
          <span
            className="input-prompt"
            style={{ color: promptColor }}
          >
            $
          </span>
          <input
            ref={inputRef}
            type="text"
            className="chat-input"
            placeholder="describe a change to your circuit..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            disabled={isLoading}
          />
          <button
            type="submit"
            className={`run-button ${input.trim() ? 'has-input' : ''}`}
            disabled={isLoading}
          >
            run
          </button>
        </div>
        {activeFile && (
          <div className="input-footer">
            <span>editing: {activeFile}</span>
          </div>
        )}
      </form>
    </div>
  );
}
