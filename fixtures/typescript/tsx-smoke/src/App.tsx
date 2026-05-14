// App.tsx — minimal TSX component (no React dependency)

interface CardProps {
  title: string;
  content: string;
}

function Card({ title, content }: CardProps): string {
  return `<div><h1>${title}</h1><p>${content}</p></div>`;
}

function App(): string {
  return Card({ title: "Hello", content: "World" });
}

export { App, Card };
