import { getCurrentWindow } from '@tauri-apps/api/window';
import './TitleBar.css';

interface TitleBarProps {
  folderName?: string | null;
  activeFile?: string | null;
}

export function TitleBar({ folderName, activeFile }: TitleBarProps) {
  const appWindow = getCurrentWindow();

  const handleMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('.traffic-light')) return;
    e.preventDefault();
    appWindow.startDragging();
  };

  const handleDoubleClick = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('.traffic-light')) return;
    appWindow.toggleMaximize();
  };

  return (
    <div className="title-bar" onMouseDown={handleMouseDown} onDoubleClick={handleDoubleClick}>
      <div className="traffic-lights">
        <button
          className="traffic-light close"
          onClick={() => appWindow.close()}
        />
        <button
          className="traffic-light minimize"
          onClick={() => appWindow.minimize()}
        />
        <button
          className="traffic-light maximize"
          onClick={() => appWindow.toggleMaximize()}
        />
      </div>
      <span className="title-bar-text">
        {folderName || 'spicy'}{activeFile ? ` — ${activeFile}` : ''}
      </span>
      <div className="title-bar-spacer" />
    </div>
  );
}
