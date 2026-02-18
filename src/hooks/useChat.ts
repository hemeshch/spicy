import { useState, useCallback, useRef, useEffect } from 'react';
import { invoke, Channel } from '@tauri-apps/api/core';
import type { ChatMessage } from '../components/Message';

interface StreamEvent {
  type: 'thinking' | 'text' | 'done' | 'error';
  content?: string;
  message?: string;
  explanation?: string;
  changes?: { component?: string; filename: string; description: string }[];
}

export interface ChatSessionMeta {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  message_count: number;
}

interface StoredMessage {
  id: string;
  role: string;
  content: string;
  thinking?: string;
  changes?: { component?: string; filename: string; description: string }[];
}

interface SessionData {
  id: string;
  title: string;
  messages: StoredMessage[];
}

interface FileChatState {
  sessions: ChatSessionMeta[];
  activeSessionId: string | null;
  messages: ChatMessage[];
}

function toStoredMessages(msgs: ChatMessage[]): StoredMessage[] {
  return msgs.map((m) => ({
    id: m.id,
    role: m.role,
    content: m.content,
    ...(m.thinking ? { thinking: m.thinking } : {}),
    ...(m.changes ? { changes: m.changes } : {}),
  }));
}

function toChatMessages(stored: StoredMessage[]): ChatMessage[] {
  return stored.map((m) => ({
    ...m,
    role: m.role as 'user' | 'assistant',
  }));
}

export function useChat(activeFile: string | null) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [sessions, setSessions] = useState<ChatSessionMeta[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

  const messagesRef = useRef(messages);
  messagesRef.current = messages;

  const sessionsRef = useRef(sessions);
  sessionsRef.current = sessions;

  const activeSessionIdRef = useRef(activeSessionId);
  activeSessionIdRef.current = activeSessionId;

  const activeFileRef = useRef(activeFile);
  activeFileRef.current = activeFile;

  const cacheRef = useRef(new Map<string, FileChatState>());
  const prevFileRef = useRef<string | null>(null);

  // Track loading transitions for auto-persist
  const wasLoadingRef = useRef(false);
  const loadingForFileRef = useRef<string | null>(null);

  // Persist current session to disk
  const persistToDisk = useCallback(async (msgs: ChatMessage[]) => {
    const file = activeFileRef.current;
    if (!file || msgs.length === 0) return;

    let sessionId = activeSessionIdRef.current;
    if (!sessionId) {
      sessionId = `session-${Date.now()}`;
      setActiveSessionId(sessionId);
      activeSessionIdRef.current = sessionId;
    }

    const firstUserMsg = msgs.find((m) => m.role === 'user');
    const title = firstUserMsg
      ? firstUserMsg.content.slice(0, 50)
      : 'New chat';

    try {
      await invoke('save_chat_session', {
        file,
        session: {
          id: sessionId,
          title,
          messages: toStoredMessages(msgs),
        },
      });
      const index = await invoke<{ sessions: ChatSessionMeta[] }>(
        'list_chat_sessions',
        { file }
      );
      setSessions(index.sessions);
      sessionsRef.current = index.sessions;
    } catch (e) {
      console.error('Failed to persist session:', e);
    }
  }, []);

  // Auto-persist when loading completes (done event)
  useEffect(() => {
    if (wasLoadingRef.current && !isLoading && messages.length > 0) {
      if (loadingForFileRef.current === activeFileRef.current) {
        persistToDisk(messages);
      }
      loadingForFileRef.current = null;
    }
    wasLoadingRef.current = isLoading;
  }, [isLoading, messages, persistToDisk]);

  // Handle activeFile changes
  useEffect(() => {
    // Save previous file state to cache
    if (prevFileRef.current && prevFileRef.current !== activeFile) {
      cacheRef.current.set(prevFileRef.current, {
        sessions: sessionsRef.current,
        activeSessionId: activeSessionIdRef.current,
        messages: messagesRef.current,
      });
    }
    prevFileRef.current = activeFile;

    if (!activeFile) {
      setMessages([]);
      setSessions([]);
      setActiveSessionId(null);
      return;
    }

    // Check cache first for instant tab switching
    const cached = cacheRef.current.get(activeFile);
    if (cached) {
      setSessions(cached.sessions);
      setActiveSessionId(cached.activeSessionId);
      setMessages(cached.messages);
      return;
    }

    // Load from disk
    (async () => {
      try {
        const index = await invoke<{ sessions: ChatSessionMeta[] }>(
          'list_chat_sessions',
          { file: activeFile }
        );
        setSessions(index.sessions);

        if (index.sessions.length > 0) {
          const latest = index.sessions[0];
          const data = await invoke<SessionData>('load_chat_session', {
            file: activeFile,
            sessionId: latest.id,
          });
          setActiveSessionId(latest.id);
          setMessages(toChatMessages(data.messages));
        } else {
          setActiveSessionId(null);
          setMessages([]);
        }
      } catch {
        setSessions([]);
        setActiveSessionId(null);
        setMessages([]);
      }
    })();
  }, [activeFile]);

  const sendMessage = useCallback(
    async (content: string) => {
      loadingForFileRef.current = activeFileRef.current;

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
          activeFile: activeFileRef.current,
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
    [persistToDisk]
  );

  const switchSession = useCallback(async (sessionId: string) => {
    const file = activeFileRef.current;
    if (!file || sessionId === activeSessionIdRef.current) return;

    try {
      const data = await invoke<SessionData>('load_chat_session', {
        file,
        sessionId,
      });
      setActiveSessionId(sessionId);
      activeSessionIdRef.current = sessionId;
      setMessages(toChatMessages(data.messages));
    } catch (e) {
      console.error('Failed to load session:', e);
    }
  }, []);

  const newSession = useCallback(() => {
    setActiveSessionId(null);
    activeSessionIdRef.current = null;
    setMessages([]);
  }, []);

  return {
    messages,
    isLoading,
    sendMessage,
    sessions,
    activeSessionId,
    switchSession,
    newSession,
  };
}
