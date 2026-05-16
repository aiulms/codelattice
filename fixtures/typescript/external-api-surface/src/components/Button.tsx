// Exported UI component — referenced in package.json exports
export function Button(props: { label: string }): string {
  return `<button>${props.label}</button>`;
}

export function Input(props: { placeholder: string }): string {
  return `<input placeholder="${props.placeholder}" />`;
}
