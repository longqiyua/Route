// 文件路径: src/main/java/com/tickban/dashboard/exception/GlobalExceptionHandler.java

/*
package com.tickban.dashboard.exception;

import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

@RestControllerAdvice
public class GlobalExceptionHandler {

    // 捕获所有 IllegalArgumentException（比如你之前抛的）
    @ExceptionHandler(IllegalArgumentException.class)
    public ResponseEntity<String> handleIllegalArgumentException(IllegalArgumentException e) {
        return ResponseEntity.badRequest().body(e.getMessage());
    }

    // 捕获其他未预期的异常（防止 500）
    @ExceptionHandler(Exception.class)
    public ResponseEntity<String> handleGenericException(Exception e) {
        // 注意：生产环境不要返回 e.getMessage()，避免信息泄露
        return ResponseEntity.status(HttpStatus.BAD_REQUEST).body("请求处理失败，请检查输入");
    }
}
*/
