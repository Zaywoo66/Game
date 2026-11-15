import { useEffect, useRef, useState } from 'react';
import { MainMenu } from './game/MainMenu';
import { useGameHost } from './game/host';
import { useLan } from './game/lan';
import { MapPage } from './map/MapPage';

export function App({ directMap = false }: { directMap?: boolean }) {
  const lan = useLan(true);
  const [singlePlayerStarted, setSinglePlayerStarted] = useState(false);
  const previousMode = useRef(lan.status?.mode ?? 'solo');
  const inRoom = lan.status?.mode !== undefined && lan.status.mode !== 'solo';
  const campaignStarted = directMap || singlePlayerStarted || Boolean(inRoom && lan.status?.started);
  const hostGame = useGameHost(true, lan.status?.generation ?? 0,
    campaignStarted && (lan.status?.mode !== 'guest' || lan.status.state === 'connected'),
    lan.status?.mode !== 'guest');
  const gameError = hostGame.connectionError || hostGame.commandError || (!hostGame.connected
    ? 'Open the desktop game to load its campaign data.' : '');

  useEffect(() => {
    const mode = lan.status?.mode ?? 'solo';
    if (previousMode.current !== 'solo' && mode === 'solo' && !directMap) setSinglePlayerStarted(false);
    previousMode.current = mode;
  }, [directMap, lan.status?.mode]);

  if (campaignStarted) return <MapPage lan={lan} hostGame={hostGame} />;
  return <MainMenu game={hostGame.snapshot} gameError={gameError} lan={lan}
    onSinglePlayer={(countryId) => {
      void hostGame.startSoloCampaign(countryId).then((started) => {
        if (started) setSinglePlayerStarted(true);
      });
    }} />;
}
