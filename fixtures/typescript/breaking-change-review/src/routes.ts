// Framework route handlers — file-based routes
export async function GET() {
  return Response.json({ status: "ok" });
}

export function updateUser(id: string, data: Record<string, unknown>) {
  return { id, ...data, updated: true };
}
