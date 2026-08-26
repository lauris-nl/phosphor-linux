import './styles/globals.css';
import { MainLayout } from './components/layout/MainLayout';
import { MatrixRain } from './components/matrix/MatrixRain';
import { CrtOverlay } from './components/matrix/CrtOverlay';
import { WizardProvider } from './hooks/WizardProvider';
import { SettingsProvider } from './hooks/useSettings';
import { TerminalLogProvider } from './hooks/useTerminalLog';

function App() {
  return (
    <>
      <MatrixRain rainState="idle" />
      <WizardProvider>
        <SettingsProvider>
          <TerminalLogProvider>
            <MainLayout />
          </TerminalLogProvider>
        </SettingsProvider>
      </WizardProvider>
      <CrtOverlay />
    </>
  );
}

export default App;
