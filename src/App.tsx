import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { TitleBar } from './components/TitleBar';
import { FileTabs, getTabColor } from './components/FileTabs';
import { Chat } from './components/Chat';
import { useChat } from './hooks/useChat';
import './App.css';

type AppView = 'welcome' | 'chat';

export default function App() {
  const [view, setView] = useState<AppView>('welcome');
  const [files, setFiles] = useState<string[]>([]);
  const [activeTabIndex, setActiveTabIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [apiKeySaved, setApiKeySaved] = useState(false);

  const activeFile = files.length > 0 ? files[activeTabIndex] : null;
  const promptColor = getTabColor(activeTabIndex);
  const { messages, isLoading, sendMessage } = useChat(activeFile);

  const handleSaveApiKey = useCallback(async () => {
    if (!apiKey.trim()) return;
    try {
      await invoke('set_api_key', { key: apiKey.trim() });
      setApiKeySaved(true);
    } catch (e) {
      setError(`Failed to save API key: ${e}`);
    }
  }, [apiKey]);

  const handleOpenFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true });
      if (!selected) return;

      await invoke('set_working_directory', { path: selected });
      const ascFiles = await invoke<string[]>('list_asc_files');

      if (ascFiles.length === 0) {
        setError('No .asc files found in this directory or its subdirectories.');
        return;
      }

      setFiles(ascFiles);
      setActiveTabIndex(0);
      setError(null);
      setView('chat');
    } catch (e) {
      setError(`Failed to open folder: ${e}`);
    }
  }, []);

  useEffect(() => {
    if (error) {
      const timer = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(timer);
    }
  }, [error]);

  return (
    <div className="app">
      <TitleBar />
      <div className="rainbow-stripe" />

      {view === 'welcome' ? (
        <div className="welcome">
          <div className="welcome-content">
            <h1 className="welcome-title">spicy</h1>
            <p className="welcome-subtitle">
              your AI copilot for LTspice
            </p>

            {!apiKeySaved && (
              <div className="api-key-section">
                <div className="api-key-input-row">
                  <input
                    type="password"
                    className="api-key-input"
                    placeholder="sk-ant-..."
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleSaveApiKey()}
                  />
                  <button
                    className="api-key-btn"
                    onClick={handleSaveApiKey}
                    disabled={!apiKey.trim()}
                  >
                    save
                  </button>
                </div>
                <p className="api-key-hint">
                  paste your Anthropic API key, or set ANTHROPIC_API_KEY env var
                </p>
              </div>
            )}

            {apiKeySaved && (
              <p className="api-key-saved">API key saved</p>
            )}

            <button className="open-folder-btn" onClick={handleOpenFolder}>
              open project folder
            </button>
            {error && <p className="error-text">{error}</p>}
          </div>
        </div>
      ) : (
        <div className="main-view">
          <FileTabs
            files={files}
            activeIndex={activeTabIndex}
            onSelect={setActiveTabIndex}
          />
          <Chat
            messages={messages}
            isLoading={isLoading}
            onSend={sendMessage}
            promptColor={promptColor}
          />
        </div>
      )}
    </div>
  );
}
