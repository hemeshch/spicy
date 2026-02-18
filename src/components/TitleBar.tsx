import { getCurrentWindow } from '@tauri-apps/api/window';
import './TitleBar.css';

export function TitleBar() {
  const appWindow = getCurrentWindow();

  return (
    <div className="title-bar" data-tauri-drag-region>
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
      <span className="title-bar-text">spicy</span>
      <div className="title-bar-spacer" />
    </div>
  );
}
