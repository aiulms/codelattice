// Next.js-like route handlers — file-based API routes
export async function GET() {
  return Response.json({ status: "ok" });
}

export async function POST(request: Request) {
  const body = await request.json();
  return Response.json({ received: body });
}

// Not a framework handler — internal helper
export function helperLoad(): string {
  return "loaded";
}
