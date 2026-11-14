import os
from contextlib import asynccontextmanager
import asyncpg
from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse, JSONResponse

@asynccontextmanager
async def lifespan(app: FastAPI):
    DB_PASSWORD = os.getenv("DB_PASSWORD")
    DB_USER=os.getenv("DB_USER")
    DB_NAME=os.getenv("DB_NAME")

    print(f"Login = {DB_USER}, Db = {DB_NAME}", flush=True)

    app.state.pool = await asyncpg.create_pool(
        user=DB_USER, 
        database=DB_NAME, 
        password=DB_PASSWORD,
        host="pg_database",
        port=5432
    )
    yield
    await app.state.pool.close()

app = FastAPI(lifespan=lifespan)

@app.get("/", response_class=HTMLResponse)
async def hello_world():
    return "<p>Service running<br/>\nTo access DB open an <a href='/api'>api</a> endpoint</p>"

@app.get("/api", response_class=JSONResponse)
async def read_data(request: Request):
    async with request.app.state.pool.acquire() as conn:
        # inline SQL for simplicity
        rows = await conn.fetch("SELECT item_id, name FROM DemoItems ORDER BY name")
        return [dict(row) for row in rows]
