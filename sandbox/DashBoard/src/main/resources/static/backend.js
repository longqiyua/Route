// backend.js
const express = require('express');
const cors = require('cors');
const fs = require('fs');
const path = require('path');

const app = express();
const PORT = 5000;
const DB_FILE = path.join(__dirname, 'tasks.json');

app.use(cors());
app.use(express.json());

// 讀取任務
app.get('/api/tasks', (req, res) => {
    if (!fs.existsSync(DB_FILE)) {
        return res.json([]);
    }
    const data = fs.readFileSync(DB_FILE, 'utf8');
    try {
        res.json(JSON.parse(data));
    } catch (e) {
        res.json([]);
    }
});

// 新增任務
app.post('/api/tasks', (req, res) => {
    let tasks = [];
    if (fs.existsSync(DB_FILE)) {
        const data = fs.readFileSync(DB_FILE, 'utf8');
        tasks = JSON.parse(data || '[]');
    }
    const newTask = { ...req.body, id: Date.now() };
    tasks.push(newTask);
    fs.writeFileSync(DB_FILE, JSON.stringify(tasks, null, 2));
    res.status(201).json(newTask);
});

// 更新任務
app.put('/api/tasks/:id', (req, res) => {
    let tasks = [];
    if (fs.existsSync(DB_FILE)) {
        const data = fs.readFileSync(DB_FILE, 'utf8');
        tasks = JSON.parse(data || '[]');
    }
    const index = tasks.findIndex(t => t.id == req.params.id);
    if (index === -1) return res.status(404).end();
    tasks[index] = { ...tasks[index], ...req.body };
    fs.writeFileSync(DB_FILE, JSON.stringify(tasks, null, 2));
    res.json(tasks[index]);
});

// 刪除任務
app.delete('/api/tasks/:id', (req, res) => {
    let tasks = [];
    if (fs.existsSync(DB_FILE)) {
        const data = fs.readFileSync(DB_FILE, 'utf8');
        tasks = JSON.parse(data || '[]');
    }
    tasks = tasks.filter(t => t.id != req.params.id);
    fs.writeFileSync(DB_FILE, JSON.stringify(tasks, null, 2));
    res.status(204).end();
});

app.listen(PORT, () => {
    console.log(`✅ 後台啟動成功！\n👉 請打開 http://localhost:8080`);
});