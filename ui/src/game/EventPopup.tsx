import { useEffect, useState } from 'react';
import { EventView, GameCommand } from './host';

type Locale = Record<string, string>;

function useEventLocale(): Locale {
  const [locale, setLocale] = useState<Locale>({});
  useEffect(() => {
    const controller = new AbortController();
    const language = navigator.language.toLowerCase().startsWith('ru') ? 'ru' : 'en';
    const load = async (code: string) => {
      const response = await fetch(import.meta.env.BASE_URL + `locales/events.${code}.json`, {
        signal: controller.signal,
      });
      if (!response.ok) throw new Error(`Event locale: HTTP ${response.status}`);
      return response.json() as Promise<unknown>;
    };
    void load(language).catch((error: unknown) => {
      if (controller.signal.aborted || language === 'en') throw error;
      return load('en');
    })
      .then((catalog) => {
        if (catalog && typeof catalog === 'object' && !Array.isArray(catalog)) {
          setLocale(Object.fromEntries(Object.entries(catalog).filter((entry): entry is [string, string] =>
            typeof entry[1] === 'string')));
        }
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted) console.error('Could not load event locale:', error);
      });
    return () => controller.abort();
  }, []);
  return locale;
}

interface Props {
  events: EventView[];
  localCountryId: number | null;
  pending: boolean;
  error: string;
  send: (command: GameCommand) => Promise<void>;
}

export function EventPopup({ events, localCountryId, pending, error, send }: Props) {
  const locale = useEventLocale();
  const localEvents = events.filter((item) => item.country_id === localCountryId);
  const event = localEvents[0];
  if (!event) return null;
  const translate = (key: string) => locale[key] ?? key;
  return (
    <div className="event-backdrop">
      <section className="event-popup" role="dialog" aria-modal="true" aria-labelledby="event-title">
        {localEvents.length > 1 && <p className="event-queue">1 of {localEvents.length} pending events</p>}
        <h2 id="event-title">{translate(event.title_key)}</h2>
        <p className="event-description">{translate(event.desc_key)}</p>
        <div className="event-options">
          {event.options.map((option, index) => (
            <button key={`${event.key}:${index}`} type="button" disabled={pending}
              onClick={() => void send({ type: 'choose_event', country_id: event.country_id,
                key: event.key, option: index })}>
              {translate(option.text_key)}
            </button>
          ))}
        </div>
        {error && <p className="game-error" role="alert">{error}</p>}
      </section>
    </div>
  );
}
