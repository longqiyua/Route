#!/usr/bin/env node

/**
 * Grill会话启动器
 * 启动新的Grill会话澄清需求、建立共同理解
 */

const fs = require('fs');
const path = require('path');
const readline = require('readline');

// Grill会话管理
class GrillSession {
  constructor(topic, userId = 'user', aiId = 'ai') {
    this.id = `grill-${Date.now()}`;
    this.topic = topic;
    this.userId = userId;
    this.aiId = aiId;
    this.createdAt = new Date().toISOString();
    this.updatedAt = this.createdAt;
    this.status = 'active'; // active, completed, cancelled
    this.decisions = [];
    this.terms = [];
    this.questions = [];
    this.tasks = [];
    this.notes = [];
  }
  
  addDecision(question, answer, rationale, requiresAdr = false) {
    const decision = {
      id: `decision-${this.decisions.length + 1}`,
      question,
      answer,
      rationale,
      requiresAdr,
      timestamp: new Date().toISOString(),
      adrCreated: false
    };
    this.decisions.push(decision);
    this.updatedAt = new Date().toISOString();
    return decision;
  }
  
  addTerm(term, definition, context = 'general') {
    const existing = this.terms.find(t => t.term === term);
    if (existing) {
      existing.definition = definition;
      existing.context = context;
      existing.updatedAt = new Date().toISOString();
    } else {
      this.terms.push({
        term,
        definition,
        context,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString()
      });
    }
    this.updatedAt = new Date().toISOString();
  }
  
  addQuestion(question, answer = '', status = 'asked') {
    this.questions.push({
      id: `question-${this.questions.length + 1}`,
      question,
      answer,
      status, // asked, answered, pending
      askedAt: new Date().toISOString(),
      answeredAt: answer ? new Date().toISOString() : null
    });
    this.updatedAt = new Date().toISOString();
  }
  
  addTask(title, description, priority = 'normal') {
    const task = {
      id: `task-${this.tasks.length + 1}`,
      title,
      description,
      priority, // critical, high, normal, low
      status: 'pending',
      createdAt: new Date().toISOString(),
      createdFrom: this.id
    };
    this.tasks.push(task);
    this.updatedAt = new Date().toISOString();
    return task;
  }
  
  complete() {
    this.status = 'completed';
    this.completedAt = new Date().toISOString();
    this.updatedAt = this.completedAt;
  }
  
  toJSON() {
    return {
      id: this.id,
      topic: this.topic,
      participants: {
        user: this.userId,
        ai: this.aiId
      },
      timestamps: {
        createdAt: this.createdAt,
        updatedAt: this.updatedAt,
        completedAt: this.completedAt
      },
      status: this.status,
      decisions: this.decisions,
      terms: this.terms,
      questions: this.questions,
      tasks: this.tasks,
      notes: this.notes,
      summary: this.generateSummary()
    };
  }
  
  generateSummary() {
    return {
      totalDecisions: this.decisions.length,
      decisionsRequiringAdr: this.decisions.filter(d => d.requiresAdr).length,
      termsClarified: this.terms.length,
      questionsAsked: this.questions.length,
      questionsAnswered: this.questions.filter(q => q.answer).length,
      tasksCreated: this.tasks.length,
      sessionDuration: this.getDuration()
    };
  }
  
  getDuration() {
    if (!this.completedAt) return null;
    const start = new Date(this.createdAt);
    const end = new Date(this.completedAt);
    return end - start; // 毫秒
  }
  
  saveToFile() {
    const sessionsDir = path.join('.grill', 'sessions');
    if (!fs.existsSync(sessionsDir)) {
      fs.mkdirSync(sessionsDir, { recursive: true });
    }
    
    const filePath = path.join(sessionsDir, `${this.id}.json`);
    fs.writeFileSync(filePath, JSON.stringify(this.toJSON(), null, 2), 'utf8');
    
    // 同时保存Markdown版本便于阅读
    const mdPath = path.join(sessionsDir, `${this.id}.md`);
    fs.writeFileSync(mdPath, this.toMarkdown(), 'utf8');
    
    return filePath;
  }
  
  toMarkdown() {
    let md = `# 🍖 Grill会话：${this.topic}\n\n`;
    
    md += `## 会话信息\n`;
    md += `- **会话ID**：${this.id}\n`;
    md += `- **主题**：${this.topic}\n`;
    md += `- **参与者**：${this.userId} + ${this.aiId}\n`;
    md += `- **开始时间**：${new Date(this.createdAt).toLocaleString()}\n`;
    md += `- **状态**：${this.status === 'active' ? '🟢 进行中' : '✅ 已完成'}\n`;
    if (this.completedAt) {
      md += `- **结束时间**：${new Date(this.completedAt).toLocaleString()}\n`;
      const duration = this.getDuration();
      if (duration) {
        const minutes = Math.floor(duration / 60000);
        md += `- **持续时间**：${minutes} 分钟\n`;
      }
    }
    
    md += `\n## 会话目标\n`;
    md += `通过结构化提问澄清需求，建立共同理解，记录决策和术语。\n`;
    
    if (this.decisions.length > 0) {
      md += `\n## 决策记录\n`;
      this.decisions.forEach((decision, index) => {
        const adrFlag = decision.requiresAdr ? ' 📋' : '';
        md += `### ${index + 1}. ${decision.question}${adrFlag}\n`;
        md += `**答案**：${decision.answer}\n`;
        md += `**理由**：${decision.rationale}\n`;
        md += `**时间**：${new Date(decision.timestamp).toLocaleTimeString()}\n\n`;
      });
    }
    
    if (this.terms.length > 0) {
      md += `\n## 术语澄清\n`;
      md += `| 术语 | 定义 | 上下文 |\n`;
      md += `|------|------|--------|\n`;
      this.terms.forEach(term => {
        md += `| ${term.term} | ${term.definition} | ${term.context} |\n`;
      });
    }
    
    if (this.questions.length > 0) {
      md += `\n## 问答记录\n`;
      this.questions.forEach((q, index) => {
        const statusIcon = q.status === 'answered' ? '✅' : q.status === 'pending' ? '⏳' : '❓';
        md += `${statusIcon} **Q${index + 1}**：${q.question}\n`;
        if (q.answer) {
          md += `   **A**：${q.answer}\n`;
        }
        md += `\n`;
      });
    }
    
    if (this.tasks.length > 0) {
      md += `\n## 生成的任务\n`;
      this.tasks.forEach((task, index) => {
        const priorityEmoji = {
          critical: '🔴',
          high: '🟡',
          normal: '🟢',
          low: '⚪'
        }[task.priority] || '🟢';
        
        md += `${priorityEmoji} **任务${index + 1}**：${task.title}\n`;
        md += `   描述：${task.description}\n`;
        md += `   状态：${task.status}\n\n`;
      });
    }
    
    if (this.notes.length > 0) {
      md += `\n## 附加笔记\n`;
      this.notes.forEach((note, index) => {
        md += `${index + 1}. ${note}\n`;
      });
    }
    
    const summary = this.generateSummary();
    md += `\n## 会话统计\n`;
    md += `- 总决策数：${summary.totalDecisions}\n`;
    md += `- 需要ADR的决策：${summary.decisionsRequiringAdr}\n`;
    md += `- 澄清的术语：${summary.termsClarified}\n`;
    md += `- 提出的问题：${summary.questionsAsked}\n`;
    md += `- 已回答问题：${summary.questionsAnswered}\n`;
    md += `- 创建的任务：${summary.tasksCreated}\n`;
    if (summary.sessionDuration) {
      md += `- 会话时长：${Math.floor(summary.sessionDuration / 60000)}分钟\n`;
    }
    
    md += `\n---\n`;
    md += `**会话记录生成时间**：${new Date().toLocaleString()}\n`;
    md += `**Grill系统版本**：2.0.0\n`;
    
    return md;
  }
}

// Grill会话管理器
class GrillManager {
  constructor() {
    this.sessionsDir = path.join('.grill', 'sessions');
    this.contextPath = path.join('.grill', 'context.md');
    this.adrsDir = path.join('.grill', 'adrs');
    this.ensureDirectories();
  }
  
  ensureDirectories() {
    if (!fs.existsSync('.grill')) {
      console.log('❌ Grill系统未初始化，请先运行: npm run grill:init');
      process.exit(1);
    }
    
    [this.sessionsDir, this.adrsDir].forEach(dir => {
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
    });
  }
  
  loadContext() {
    if (!fs.existsSync(this.contextPath)) {
      return { terms: [] };
    }
    
    const content = fs.readFileSync(this.contextPath, 'utf8');
    // 简单解析context.md，实际应该更复杂
    return { content };
  }
  
  updateContext(newTerms) {
    if (!fs.existsSync(this.contextPath)) return;
    
    let content = fs.readFileSync(this.contextPath, 'utf8');
    const lines = content.split('\n');
    
    // 找到术语表部分
    let termsSectionIndex = -1;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].includes('术语澄清') || lines[i].includes('## 术语')) {
        termsSectionIndex = i;
        break;
      }
    }
    
    if (termsSectionIndex === -1) {
      // 如果没有术语部分，添加到文件末尾
      content += '\n\n## 术语澄清\n';
      newTerms.forEach(term => {
        content += `- **${term.term}**：${term.definition}（${term.context}）\n`;
      });
    } else {
      // 在术语部分后添加新术语
      const beforeTerms = lines.slice(0, termsSectionIndex + 1).join('\n');
      const afterTerms = lines.slice(termsSectionIndex + 1).join('\n');
      
      let newContent = beforeTerms + '\n';
      newTerms.forEach(term => {
        newContent += `- **${term.term}**：${term.definition}（${term.context}）\n`;
      });
      newContent += '\n' + afterTerms;
      content = newContent;
    }
    
    fs.writeFileSync(this.contextPath, content, 'utf8');
  }
  
  createAdr(decision, sessionId) {
    if (!fs.existsSync(this.adrsDir)) {
      fs.mkdirSync(this.adrsDir, { recursive: true });
    }
    
    const adrNumber = fs.readdirSync(this.adrsDir)
      .filter(f => f.endsWith('.md') && f.startsWith('adr-'))
      .length + 1;
    
    const adrId = `adr-${adrNumber.toString().padStart(3, '0')}`;
    const adrPath = path.join(this.adrsDir, `${adrId}.md`);
    
    const adrContent = `# [${adrId.toUpperCase()}] ${decision.question}

## 状态
提议

## 上下文
来自Grill会话 ${sessionId}
${decision.rationale}

## 决策
${decision.answer}

## 后果
### 正面影响
- 决策具体化，便于实施

### 负面影响
- 需要评估具体实施风险

## 相关链接
- Grill会话：${sessionId}
- 决策时间：${new Date(decision.timestamp).toLocaleString()}

---

**决策时间**：${decision.timestamp.split('T')[0]}
**决策者**：Grill会话参与者
**记录时间**：${new Date().toISOString().split('T')[0]}
`;
    
    fs.writeFileSync(adrPath, adrContent, 'utf8');
    return adrId;
  }
  
  listSessions() {
    if (!fs.existsSync(this.sessionsDir)) return [];
    
    return fs.readdirSync(this.sessionsDir)
      .filter(f => f.endsWith('.json'))
      .map(f => {
        const filePath = path.join(this.sessionsDir, f);
        try {
          const content = fs.readFileSync(filePath, 'utf8');
          return JSON.parse(content);
        } catch (e) {
          return null;
        }
      })
      .filter(s => s !== null)
      .sort((a, b) => new Date(b.timestamps.createdAt) - new Date(a.timestamps.createdAt));
  }
}

// 交互式Grill会话
async function interactiveGrillSession(topic) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });
  
  const question = (query) => new Promise(resolve => rl.question(query, resolve));
  
  console.log(`\n🍖 开始Grill会话：${topic}\n`);
  console.log('='.repeat(50));
  
  const session = new GrillSession(topic);
  const manager = new GrillManager();
  
  try {
    console.log('🎯 首先，让我们明确会话目标...\n');
    
    // 1. 明确范围
    const scope = await question('📋 这个功能的边界是什么？（输入完成继续）\n> ');
    session.addQuestion('功能边界是什么？', scope, 'answered');
    
    // 2. 核心决策点
    console.log('\n🔍 现在探索核心决策点...\n');
    
    let continueExploring = true;
    while (continueExploring) {
      const decisionQ = await question('❓ 下一个关键决策点是什么？（输入"完成"结束）\n> ');
      
      if (decisionQ.toLowerCase() === '完成' || decisionQ === '') {
        continueExploring = false;
        continue;
      }
      
      const answer = await question(`💡 建议答案：`);
      const rationale = await question(`🤔 选择这个答案的理由：`);
      const needsAdr = (await question(`📋 需要创建ADR记录吗？(y/n)：`)).toLowerCase() === 'y';
      
      session.addDecision(decisionQ, answer, rationale, needsAdr);
      console.log(`✅ 已记录决策：${decisionQ}\n`);
    }
    
    // 3. 术语澄清
    console.log('\n🔤 现在澄清重要术语...\n');
    
    let continueTerms = true;
    while (continueTerms) {
      const term = await question('📖 需要澄清的术语是什么？（输入"完成"结束）\n> ');
      
      if (term.toLowerCase() === '完成' || term === '') {
        continueTerms = false;
        continue;
      }
      
      const definition = await question(`📝 ${term}的定义：`);
      const context = await question(`🏷️  使用上下文：`);
      
      session.addTerm(term, definition, context);
      console.log(`✅ 已记录术语：${term}\n`);
    }
    
    // 4. 生成任务
    console.log('\n🎯 从决策创建任务...\n');
    
    for (const decision of session.decisions) {
      const createTask = (await question(`📋 从"${decision.question}"创建任务吗？(y/n)：`)).toLowerCase() === 'y';
      
      if (createTask) {
        const taskTitle = await question(`  任务标题：`);
        const taskDesc = await question(`  任务描述：`);
        const priority = await question(`  优先级(critical/high/normal/low)：`) || 'normal';
        
        session.addTask(taskTitle, taskDesc, priority);
        console.log(`   ✅ 已创建任务：${taskTitle}\n`);
      }
    }
    
    // 完成会话
    session.complete();
    
    // 保存会话
    const savedPath = session.saveToFile();
    
    // 更新context.md
    if (session.terms.length > 0) {
      manager.updateContext(session.terms);
    }
    
    // 创建ADR
    for (const decision of session.decisions.filter(d => d.requiresAdr)) {
      const adrId = manager.createAdr(decision, session.id);
      console.log(`📋 已创建ADR：${adrId}`);
    }
    
    console.log('\n' + '='.repeat(50));
    console.log(`✅ Grill会话完成！\n`);
    
    const summary = session.generateSummary();
    console.log(`📊 会话统计:`);
    console.log(`   决策数：${summary.totalDecisions}`);
    console.log(`   术语澄清：${summary.termsClarified}`);
    console.log(`   任务创建：${summary.tasksCreated}`);
    console.log(`   会话时长：${Math.floor(summary.sessionDuration / 60000)}分钟`);
    
    console.log(`\n📁 产出位置:`);
    console.log(`   会话记录：${savedPath}`);
    console.log(`   术语表：${manager.contextPath}`);
    if (session.decisions.some(d => d.requiresAdr)) {
      console.log(`   ADRs：${manager.adrsDir}/`);
    }
    
    console.log(`\n🎯 下一步:`);
    console.log(`   1. 查看会话记录：cat ${savedPath}`);
    console.log(`   2. 开始实施任务：npm run task:create`);
    console.log(`   3. 验证项目结构：npm run validate`);
    
  } catch (error) {
    console.error('❌ Grill会话出错:', error.message);
  } finally {
    rl.close();
  }
}

// 快速Grill模式（非交互式）
function quickGrillMode(topic) {
  console.log(`\n⚡ 快速Grill模式：${topic}\n`);
  
  const session = new GrillSession(topic);
  const manager = new GrillManager();
  
  // 添加示例内容
  session.addDecision(
    '这个功能的核心目标是什么？',
    '通过结构化提问澄清需求',
    '明确目标是有效Grill的前提',
    false
  );
  
  session.addDecision(
    '需要什么样的产出？',
    '术语表更新、任务列表、可能的ADR',
    '明确的产出定义确保会话有效性',
    true
  );
  
  session.addTerm('Grill', '无情的提问澄清过程', '需求分析');
  session.addTerm('ADR', '架构决策记录', '技术决策');
  
  session.addTask(
    '实现Grill会话记录功能',
    '创建会话记录保存和检索系统',
    'high'
  );
  
  session.complete();
  session.saveToFile();
  
  // 更新context.md
  if (session.terms.length > 0) {
    manager.updateContext(session.terms);
  }
  
  console.log('✅ 快速Grill会话已创建');
  console.log(`   会话ID：${session.id}`);
  console.log(`   产出位置：.grill/sessions/${session.id}.json`);
  
  return session;
}

// 主函数
function main() {
  const args = process.argv.slice(2);
  
  if (args.length === 0) {
    console.log(`
🍖 Grill会话启动器

用法:
  npm run grill:start "会话主题"         启动交互式Grill
  npm run grill:start "主题" --quick     快速Grill模式
  node scripts/grill-start.js --help     显示帮助

模式:
  --interactive (默认)   交互式Grill，逐步提问
  --quick               快速Grill，使用预设模板
  --template=<name>     使用特定模板

示例:
  npm run grill:start "用户认证系统设计"
  npm run grill:start "API设计" --quick
  npm run grill:start "数据库架构" --template=db-design

产出:
  .grill/sessions/<id>.json   结构化会话数据
  .grill/sessions/<id>.md     可读的会话记录
  .grill/context.md           更新的术语表
  .grill/adrs/*.md            架构决策记录
  tasks/*.md                  生成的任务文件
`);
    return;
  }
  
  // 检查Grill系统是否初始化
  if (!fs.existsSync('.grill')) {
    console.log('❌ Grill系统未初始化，请先运行: npm run grill:init');
    process.exit(1);
  }
  
  // 解析参数
  const topic = args[0];
  const isQuick = args.includes('--quick');
  const isInteractive = args.includes('--interactive') || (!isQuick && !args.includes('--template'));
  
  if (isInteractive) {
    interactiveGrillSession(topic).catch(console.error);
  } else if (isQuick) {
    quickGrillMode(topic);
  } else if (args.some(arg => arg.startsWith('--template='))) {
    const templateArg = args.find(arg => arg.startsWith('--template='));
    const templateName = templateArg.split('=')[1];
    console.log(`📋 使用模板: ${templateName}`);
    quickGrillMode(topic); // 简化处理
  }
}

// 导出供测试使用
module.exports = {
  GrillSession,
  GrillManager,
  interactiveGrillSession,
  quickGrillMode
};

// 运行主函数
if (require.main === module) {
  main();
}