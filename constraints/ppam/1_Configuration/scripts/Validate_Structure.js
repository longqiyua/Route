#!/usr/bin/env node

/**
 * 精简版项目结构验证器
 * 验证Grill驱动的项目结构
 */

const fs = require('fs');
const path = require('path');

// 必需的核心文件夹
const REQUIRED_FOLDERS = [
  '.grill',    // Grill系统工作空间
  'src',       // 源代码
  'docs',      // 文档
  'tasks',     // 任务管理
  'scripts',   // 工具脚本
  'config',    // 配置文件
  'data',      // 数据文件
];

// 建议的文件夹
const RECOMMENDED_FOLDERS = [
  'tests',     // 测试文件
  'temp',      // 临时文件
];

// 允许的根目录文件
const ALLOWED_ROOT_FILES = [
  'package.json',
  'README.md',
  'QUICK-START.md',
  'LICENSE',
  '.gitignore',
  '.github',
  '.grill',
];

// 必需的核心文件
const REQUIRED_FILES = {
  '.grill': ['context.md', 'README.md'],
  'scripts': ['validate-structure.js', 'grill-init.js'],
};

// 验证函数
function validateProjectStructure() {
  console.log('🔍 验证Grill项目结构...\n');
  
  let hasErrors = false;
  let warnings = [];
  
  // 1. 验证必需文件夹
  console.log('📁 验证核心文件夹:');
  for (const folder of REQUIRED_FOLDERS) {
    const exists = fs.existsSync(folder) && fs.statSync(folder).isDirectory();
    if (exists) {
      console.log(`   ✅ ${folder}/`);
    } else {
      console.log(`   ❌ ${folder}/ - 必需文件夹不存在`);
      hasErrors = true;
    }
  }
  
  // 2. 验证建议文件夹
  console.log('\n📁 检查建议文件夹:');
  for (const folder of RECOMMENDED_FOLDERS) {
    const exists = fs.existsSync(folder) && fs.statSync(folder).isDirectory();
    if (exists) {
      console.log(`   ✅ ${folder}/`);
    } else {
      console.log(`   ⚠️  ${folder}/ - 建议添加此文件夹`);
      warnings.push(`建议添加文件夹: ${folder}/`);
    }
  }
  
  // 3. 验证根目录文件
  console.log('\n📄 检查根目录文件:');
  const rootFiles = fs.readdirSync('.').filter(file => {
    const filePath = path.join('.', file);
    return fs.statSync(filePath).isFile();
  });
  
  for (const file of rootFiles) {
    if (ALLOWED_ROOT_FILES.includes(file) || 
        ALLOWED_ROOT_FILES.some(allowed => file.startsWith(allowed + '/'))) {
      console.log(`   ✅ ${file} (允许的文件)`);
    } else if (file.startsWith('.')) {
      // 隐藏文件可能被允许
      console.log(`   ⚠️  ${file} (隐藏文件)`);
    } else {
      console.log(`   ❌ ${file} - 请移动到合适的文件夹`);
      hasErrors = true;
    }
  }
  
  // 4. 验证必需文件
  console.log('\n📄 验证必需文件:');
  for (const [folder, files] of Object.entries(REQUIRED_FILES)) {
    if (fs.existsSync(folder)) {
      for (const file of files) {
        const filePath = path.join(folder, file);
        if (fs.existsSync(filePath)) {
          console.log(`   ✅ ${folder}/${file}`);
        } else {
          console.log(`   ❌ ${folder}/${file} - 必需文件不存在`);
          hasErrors = true;
        }
      }
    }
  }
  
  // 5. 检查.grill结构
  console.log('\n🍖 检查Grill系统:');
  const grillPath = '.grill';
  if (fs.existsSync(grillPath)) {
    const grillContents = fs.readdirSync(grillPath);
    
    // 检查context.md
    if (grillContents.includes('context.md')) {
      console.log('   ✅ .grill/context.md 存在');
    } else {
      console.log('   ⚠️  .grill/context.md 不存在 - 运行 npm run grill:init');
      warnings.push('缺少 .grill/context.md - 运行 npm run grill:init');
    }
    
    // 检查adrs文件夹
    if (grillContents.includes('adrs') || fs.existsSync(path.join(grillPath, 'adrs'))) {
      console.log('   ✅ .grill/adrs/ 文件夹存在');
    } else {
      console.log('   ⚠️  .grill/adrs/ 不存在 - 将自动创建');
    }
    
    // 检查sessions文件夹
    if (grillContents.includes('sessions') || fs.existsSync(path.join(grillPath, 'sessions'))) {
      console.log('   ✅ .grill/sessions/ 文件夹存在');
    } else {
      console.log('   ⚠️  .grill/sessions/ 不存在 - 将自动创建');
    }
  } else {
    console.log('   ❌ .grill/ 文件夹不存在 - 运行 npm run grill:init');
    hasErrors = true;
  }
  
  // 6. 检查tasks文件夹
  console.log('\n📋 检查任务管理:');
  const tasksPath = 'tasks';
  if (fs.existsSync(tasksPath)) {
    const taskFiles = fs.readdirSync(tasksPath).filter(f => f.endsWith('.md'));
    
    if (taskFiles.length > 0) {
      console.log(`   ✅ tasks/ 中有 ${taskFiles.length} 个任务文件`);
      taskFiles.slice(0, 3).forEach(file => {
        console.log(`      - ${file}`);
      });
      if (taskFiles.length > 3) {
        console.log(`      ... 还有 ${taskFiles.length - 3} 个文件`);
      }
    } else {
      console.log('   ⚠️  tasks/ 文件夹为空 - 还没有任务');
    }
  } else {
    console.log('   ⚠️  tasks/ 文件夹不存在 - 将自动创建');
  }
  
  // 显示结果
  console.log('\n' + '='.repeat(50));
  
  if (hasErrors) {
    console.log('❌ 结构验证失败！\n');
    console.log('修复建议:');
    console.log('1. 运行 npm run structure:init 初始化基本结构');
    console.log('2. 运行 npm run grill:init 初始化Grill系统');
    console.log('3. 将文件移动到合适的文件夹');
    console.log('4. 重新运行验证');
    process.exit(1);
  } else {
    console.log('✅ 项目结构验证通过！\n');
    
    if (warnings.length > 0) {
      console.log('⚠️  警告:');
      warnings.forEach(warning => console.log(`   - ${warning}`));
      console.log();
    }
    
    console.log('🎯 下一步:');
    console.log('1. 如有警告，按建议修复');
    console.log('2. 运行 npm run grill:start 开始第一个Grill会话');
    console.log('3. 运行 npm run task:create 创建任务');
    console.log('4. 开始开发！');
    
    // 显示项目概览
    console.log('\n📊 项目概览:');
    const totalFiles = countFiles('.');
    console.log(`   总文件数: ${totalFiles}`);
    console.log(`   核心文件夹: ${REQUIRED_FOLDERS.length} 个`);
    console.log(`   Grill状态: ${fs.existsSync('.grill/context.md') ? '已初始化' : '未初始化'}`);
    console.log(`   任务数量: ${fs.existsSync('tasks') ? fs.readdirSync('tasks').length : 0}`);
  }
}

// 统计文件数量
function countFiles(dir) {
  let count = 0;
  
  function traverse(currentDir) {
    const items = fs.readdirSync(currentDir);
    
    for (const item of items) {
      const itemPath = path.join(currentDir, item);
      const stat = fs.statSync(itemPath);
      
      // 跳过.git文件夹
      if (item === '.git' && stat.isDirectory()) {
        continue;
      }
      
      if (stat.isFile()) {
        count++;
      } else if (stat.isDirectory()) {
        traverse(itemPath);
      }
    }
  }
  
  traverse(dir);
  return count;
}

// 帮助信息
function showHelp() {
  console.log(`
🍖 Grill项目结构验证器

用法:
  npm run validate         验证当前项目结构
  node scripts/validate-structure.js  直接运行验证

验证规则:
  必需文件夹:
    .grill/     Grill系统工作空间
    src/        源代码
    docs/       文档
    tasks/      任务管理
    scripts/    工具脚本
    config/     配置文件
    data/       数据文件

  建议文件夹:
    tests/      测试文件
    temp/       临时文件

  根目录允许的文件:
    package.json, README.md, LICENSE, .gitignore
    .github/, .grill/

快速开始:
  1. npm run structure:init   初始化项目结构
  2. npm run grill:init       初始化Grill系统
  3. npm run validate         验证结构
  4. npm run grill:start      开始开发

设计理念:
  通过Grill驱动的工作流，确保AI和人类之间的完美理解
  简化结构，强化核心，专注于有效协作
`);
}

// 主函数
function main() {
  const args = process.argv.slice(2);
  
  if (args.includes('--help') || args.includes('-h')) {
    showHelp();
    return;
  }
  
  validateProjectStructure();
}

// 导出供测试使用
module.exports = {
  validateProjectStructure,
  REQUIRED_FOLDERS,
  ALLOWED_ROOT_FILES
};

// 运行主函数
if (require.main === module) {
  main();
}