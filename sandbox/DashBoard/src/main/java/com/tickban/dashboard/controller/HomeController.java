package com.tickban.dashboard.controller;

import org.springframework.stereotype.Controller;
import org.springframework.web.bind.annotation.GetMapping;

@Controller//位于Control层
public class HomeController {

    @GetMapping("/")//访问目标网址调用如下函数
    public String home() {
        return "forward:/index.html"; //转发到index.html
    }
}

//启动文件