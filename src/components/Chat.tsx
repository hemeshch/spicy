import { useState, useRef, useEffect } from 'react';
import { Message, type ChatMessage } from './Message';
import './Chat.css';

interface ChatProps {
  messages: ChatMessage[];
  isLoading: boolean;
  onSend: (message: string) => void;
  promptColor: string;
}

export function Chat({ messages, isLoading, onSend, promptColor }: ChatProps) {
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

  return (
    <div className="chat">
      <div className="chat-messages">
        {messages.map((msg) => (
          <Message key={msg.id} message={msg} />
        ))}
        {isLoading && (
          <Message
            message={{
              id: 'loading',
              role: 'assistant',
              content: '',
              isLoading: true,
            }}
          />
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
            placeholder="ask spicy to modify your circuit..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            disabled={isLoading}
          />
          <button
            type="submit"
            className="run-button"
            disabled={!input.trim() || isLoading}
          >
            run
          </button>
        </div>
      </form>
    </div>
  );
}
