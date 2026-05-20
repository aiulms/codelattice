import express from 'express';

const app = express();
const PORT = process.env.PORT || 3000;

export function createServer(config = {}) {
    const server = express();

    server.use(express.json());

    server.get('/api/health', (req, res) => {
        res.json({ status: 'ok', timestamp: Date.now() });
    });

    server.get('/api/users', (req, res) => {
        res.json({ users: [] });
    });

    server.post('/api/users', (req, res) => {
        const { name, email } = req.body;
        if (!name || !email) {
            return res.status(400).json({ error: 'name and email required' });
        }
        res.status(201).json({ id: 1, name, email });
    });

    server.get('/api/users/:id', (req, res) => {
        const { id } = req.params;
        res.json({ id: parseInt(id), name: 'User' });
    });

    server.use((err, req, res, next) => {
        console.error(err.stack);
        res.status(500).json({ error: 'Internal Server Error' });
    });

    return server;
}

export function startServer(port = PORT) {
    const server = createServer();
    return server.listen(port, () => {
        console.log(`Server running on port ${port}`);
    });
}

export default app;
