// src/main/java/com/tickban/dashboard/services/TaskServices.java
package com.tickban.dashboard.services;

import com.tickban.dashboard.entity.Task;
import com.tickban.dashboard.repository.TaskRepository;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

import java.util.List;
import java.util.Optional;

@Service
public class TaskServices {

    @Autowired
    private TaskRepository taskRepository;

    public Task addTask(Task task) {
        if (task == null) {
            return null; // 或者你可以设一个错误字段，但先简单处理
        }

        if (task.getStatus() == null || task.getStatus().trim().isEmpty()) {
            task.setStatus("wd");
        }

        if (task.getName() != null) {
            task.setName(task.getName().trim());
        }
        if (task.getName() == null || task.getName().isEmpty()) {
            return null; // 表示失败
        }

        if (task.getDetail() != null) {
            task.setDetail(task.getDetail().trim());
        }

        return taskRepository.save(task);
    }

    public List<Task> getAllTasks() {
        return taskRepository.findAll();
    }

    public Task getTaskByID(Long id) {
        if (id == null || id <= 0) return null;
        Optional<Task> task = taskRepository.findById(id);
        return task.orElse(null);
    }

    public Task updateTask(Task task) {
        if (task == null || task.getId() == null || task.getId() <= 0) {
            return null;
        }

        Optional<Task> existingOpt = taskRepository.findById(task.getId());
        if (!existingOpt.isPresent()) {
            return null; // 任务不存在
        }

        Task existing = existingOpt.get();

        if (task.getName() != null) {
            String name = task.getName().trim();
            if (!name.isEmpty()) {
                existing.setName(name);
            }
        }

        if (task.getDetail() != null) {
            existing.setDetail(task.getDetail().trim());
        }

        if (task.getPriority() != null) {
            existing.setPriority(task.getPriority());
        }

        if (task.getDuedate() != null) {
            existing.setDuedate(task.getDuedate());
        }

        if (task.getStatus() != null) {
            existing.setStatus(task.getStatus().trim());
        }

        return taskRepository.save(existing);
    }

    public void deleteTask(Long id) {
        if (id != null && id > 0) {
            taskRepository.deleteById(id); // JPA 会安全处理：存在就删，不存在就当成功
        }
    }
}