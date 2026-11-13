const files = import.meta.glob('../assets/flags/*.svg', {
  eager: true,
  query: '?url',
  import: 'default',
}) as Record<string, string>;

/** A country tag selects its licensed SVG when one has been added. */
export function flagUrl(tag: string): string | undefined {
  return files[`../assets/flags/${tag}.svg`];
}
