import { useState, useCallback, useRef } from 'react';
import { invoke, Channel } from '@tauri-apps/api/core';
import type { ChatMessage } from '../components/Message';

interface StreamEvent {
  type: 'thinking' | 'text' | 'done' | 'error';
  content?: string;
  message?: string;
  explanation?: string;
  changes?: { component?: string; filename: string; description: string }[];
}

export function useChat(activeFile: string | null) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const messagesRef = useRef(messages);
  messagesRef.current = messages;

  const sendMessage = useCallback(
    async (content: string) => {
      const userMsg: ChatMessage = {
        id: `user-${Date.now()}`,
        role: 'user',
        content,
      };

      const assistantId = `assistant-${Date.now()}`;

      setMessages((prev) => [...prev, userMsg]);
      setIsLoading(true);

      // Add a placeholder assistant message for streaming
      setMessages((prev) => [
        ...prev,
        {
          id: assistantId,
          role: 'assistant',
          content: '',
          thinking: '',
          isStreaming: true,
        },
      ]);

      try {
        const history = [...messagesRef.current, userMsg].map((m) => ({
          role: m.role,
          content: m.content,
        }));

        const channel = new Channel<StreamEvent>();

        channel.onmessage = (event: StreamEvent) => {
          switch (event.type) {
            case 'thinking':
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? { ...m, thinking: (m.thinking || '') + (event.content || '') }
                    : m
                )
              );
              break;

            case 'text':
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? { ...m, content: m.content + (event.content || '') }
                    : m
                )
              );
              break;

            case 'done':
              setMessages((prev) =>
                prev.map((m) => {
                  if (m.id !== assistantId) return m;
                  return {
                    ...m,
                    // If backend sent an explanation (edit mode), replace raw JSON with it
                    content: event.explanation || m.content,
                    isStreaming: false,
                    changes: event.changes,
                  };
                })
              );
              setIsLoading(false);
              break;

            case 'error':
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? {
                        ...m,
                        content: `Error: ${event.message || 'Unknown error'}`,
                        isStreaming: false,
                      }
                    : m
                )
              );
              setIsLoading(false);
              break;
          }
        };

        await invoke('send_chat_message_stream', {
          message: content,
          activeFile,
          history,
          onEvent: channel,
        });
      } catch (error) {
        setMessages((prev) =>
          prev.map((m) =>
            m.id === assistantId
              ? {
                  ...m,
                  content: `Error: ${error}`,
                  isStreaming: false,
                }
              : m
          )
        );
        setIsLoading(false);
      }
    },
    [activeFile]
  );

  return { messages, isLoading, sendMessage };
}
