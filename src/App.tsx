import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { TitleBar } from './components/TitleBar';
import { FileTabs, getTabColor } from './components/FileTabs';
import { Chat } from './components/Chat';
import { useChat } from './hooks/useChat';
import './App.css';

const RAINBOW = ['#61BB46', '#FDB827', '#F5821F', '#E03A3E', '#963D97', '#009DDC'];

type AppView = 'welcome' | 'chat';

export default function App() {
  const [view, setView] = useState<AppView>('welcome');
  const [files, setFiles] = useState<string[]>([]);
  const [activeTabIndex, setActiveTabIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [folderName, setFolderName] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState('');
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [selectedModel, setSelectedModel] = useState('google/gemini-3.1-pro-preview');

  const activeFile = files.length > 0 ? files[activeTabIndex] : null;
  const promptColor = getTabColor(activeTabIndex);
  const { messages, isLoading, sendMessage, sessions, activeSessionId, switchSession, newSession } = useChat(activeFile, selectedModel);

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
      setFolderName(selected.split('/').pop() || selected.split('\\').pop() || selected);
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
    invoke<boolean>('has_api_key').then((has) => {
      if (has) setApiKeySaved(true);
    });
  }, []);

  useEffect(() => {
    if (error) {
      const timer = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(timer);
    }
  }, [error]);

  return (
    <div className="app">
      <div className="rainbow-stripe" />
      <TitleBar folderName={folderName} activeFile={activeFile} />

      {view === 'welcome' ? (
        <div className="welcome">
          <div className="welcome-content">
            <div className="welcome-dots">
              {RAINBOW.map((c, i) => (
                <span key={i} className="dot" style={{ background: c }} />
              ))}
            </div>
            <h1 className="welcome-title">spicy</h1>
            <p className="welcome-subtitle">
              "adding spice to LTSpice"
            </p>

{!apiKeySaved && (
              <div className="api-key-section">
                <div className="api-key-input-row">
                  <input
                    type="password"
                    className="api-key-input"
                    placeholder="sk-or-..."
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
                  paste your OpenRouter API key, or set OPENROUTER_API_KEY in .env
                </p>
              </div>
            )}

            <button className="open-folder-btn" onClick={handleOpenFolder}>
              <svg className="folder-icon" viewBox="0 0 20 18" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M2 5V3a1 1 0 0 1 1-1h4.5l2 2H17a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V5z"/>
              </svg>
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
            activeFile={activeFile}
            sessions={sessions}
            activeSessionId={activeSessionId}
            onSwitchSession={switchSession}
            onNewSession={newSession}
            selectedModel={selectedModel}
            onModelChange={setSelectedModel}
          />
        </div>
      )}

      <div className="rainbow-stripe" />
    </div>
  );
}
