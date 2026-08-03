// src/main/java/com/tickban/dashboard/controller/TaskController.java
package com.tickban.dashboard.controller;

import com.tickban.dashboard.entity.Task;
import com.tickban.dashboard.services.TaskServices;

import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@RequestMapping("/api/tasks")
public class TaskController {

    @Autowired
    private TaskServices taskServices;

    @PostMapping
    public ResponseEntity<?> createTask(@RequestBody Task task) {
        if (task == null || task.getName() == null || task.getName().trim().isEmpty()) {
            return ResponseEntity.badRequest().body("任务名称不能为空");
        }
        Task saved = taskServices.addTask(task);
        return ResponseEntity.status(HttpStatus.CREATED).body(saved);
    }

    @GetMapping
    public ResponseEntity<List<Task>> getAllTasks() {
        return ResponseEntity.ok(taskServices.getAllTasks());
    }

    @GetMapping("/{id}")
    public ResponseEntity<?> getTask(@PathVariable Long id) {
        Task task = taskServices.getTaskByID(id);
        if (task == null) {
            return ResponseEntity.notFound().build();
        }
        return ResponseEntity.ok(task);
    }

    // ✅ 修复：允许前端不传 id，用 URL 的 id 强制设置
    @PutMapping("/{id}")
    public ResponseEntity<?> updateTask(
            @PathVariable Long id,
            @RequestBody Task task) {

        if (task == null) {
            return ResponseEntity.badRequest().body("请求体不能为空");
        }

        // 强制使用 URL 中的 id，忽略前端传的 id（或补上）
        task.setId(id);

        Task updated = taskServices.updateTask(task);
        if (updated == null) {
            return ResponseEntity.notFound().build();
        }
        return ResponseEntity.ok(updated);
    }

    @DeleteMapping("/{id}")
    public ResponseEntity<Void> deleteTask(@PathVariable Long id) {
        taskServices.deleteTask(id);
        return ResponseEntity.noContent().build();
    }
}