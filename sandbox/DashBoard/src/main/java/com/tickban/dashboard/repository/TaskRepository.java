package com.tickban.dashboard.repository;

import com.tickban.dashboard.entity.Task;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

@Repository//将数据库操作交给@Repository标记的如下接口类进行执行
public interface TaskRepository extends JpaRepository<Task, Long> {//接口类，完全继承父类
}

