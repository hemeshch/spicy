import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ChatMessage } from '../components/Message';

interface ChatResponse {
  explanation: string;
  changes: { filename: string; description: string }[];
}

export function useChat(activeFile: string | null) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const sendMessage = useCallback(
    async (content: string) => {
      const userMsg: ChatMessage = {
        id: `user-${Date.now()}`,
        role: 'user',
        content,
      };

      setMessages((prev) => [...prev, userMsg]);
      setIsLoading(true);

      try {
        // Build history for context (exclude loading messages)
        const history = messages.map((m) => ({
          role: m.role,
          content: m.content,
        }));

        const response = await invoke<ChatResponse>('send_chat_message', {
          message: content,
          activeFile,
          history,
        });

        const assistantMsg: ChatMessage = {
          id: `assistant-${Date.now()}`,
          role: 'assistant',
          content: response.explanation,
          changes: response.changes,
        };

        setMessages((prev) => [...prev, assistantMsg]);
      } catch (error) {
        const errorMsg: ChatMessage = {
          id: `error-${Date.now()}`,
          role: 'assistant',
          content: `Error: ${error}`,
        };
        setMessages((prev) => [...prev, errorMsg]);
      } finally {
        setIsLoading(false);
      }
    },
    [messages, activeFile]
  );

  return { messages, isLoading, sendMessage };
}
