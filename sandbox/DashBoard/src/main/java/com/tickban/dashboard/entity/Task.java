package com.tickban.dashboard.entity;

import com.fasterxml.jackson.annotation.JsonFormat;
import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.persistence.*;
import lombok.Data;

import java.time.LocalDate;
import java.time.LocalDateTime;

@Entity
@Table(name = "task")
@Data//Lombok注解，
public class Task {
    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(nullable = false, length = 100)
    private String name;

    private String detail;

    // 👇 直接改成 String！不再用 LocalDateTime
    @JsonProperty("duedate")//控制前后端通信时的变量一致，防止通信失败
    @JsonFormat(pattern = "yyyy-MM-dd") // 注意：这里是 -，和前端一致
    private LocalDate duedate;



    private Priority priority;

    @Column(nullable = false, length = 3)
    private String status;

    @Transient
    public String getColor() {
        if (priority == null) return "gray";
        return switch (priority) {
            case HIGH -> "red";
            case MEDIUM -> "yellow";
            case LOW -> "cyan";
        };
    }
}