# Simulated FastAPI routes — decorated functions are framework entry candidates
from app import app, router

@app.get("/users/{id}")
def get_user(id: int):
    """Get user by ID — route handler"""
    return {"id": id, "name": "test"}

@router.post("/orders")
def create_order(data: dict):
    """Create a new order — route handler"""
    return {"status": "ok"}

@app.put("/users/{id}")
def update_user(id: int, data: dict):
    """Update user — route handler"""
    return {"id": id, "updated": True}

@router.get("/health")
def health_check():
    """Health check endpoint"""
    return {"status": "healthy"}
