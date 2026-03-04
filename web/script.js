// 默认提示词（从服务器获取）
let DEFAULT_SYSTEM_PROMPT = '';
let DEFAULT_SPLIT_PROMPT = '';

// 加载默认提示词
async function loadDefaults() {
    try {
        const response = await fetch('/api/defaults');
        if (response.ok) {
            const defaults = await response.json();
            DEFAULT_SYSTEM_PROMPT = defaults.system_prompt;
            DEFAULT_SPLIT_PROMPT = defaults.split_prompt;
        }
    } catch (error) {
        console.error('加载默认提示词失败:', error);
    }
}

// 重置搜索系统提示词
function resetSystemPrompt() {
    if (!DEFAULT_SYSTEM_PROMPT) {
        showStatus('❌ 默认提示词未加载', true);
        return;
    }
    document.getElementById('system_prompt').value = DEFAULT_SYSTEM_PROMPT;
    showStatus('✅ 已重置为默认搜索系统提示词');
}

// 重置查询拆分提示词
function resetSplitPrompt() {
    if (!DEFAULT_SPLIT_PROMPT) {
        showStatus('❌ 默认提示词未加载', true);
        return;
    }
    document.getElementById('split_prompt').value = DEFAULT_SPLIT_PROMPT;
    showStatus('✅ 已重置为默认查询拆分提示词');
}

// 状态显示
function showStatus(message, isError = false) {
    const status = document.getElementById('status');
    status.textContent = message;
    status.className = `status ${isError ? 'error' : 'success'}`;
    
    setTimeout(() => {
        status.className = 'status hidden';
    }, 5000);
}

// 加载配置
async function loadConfig() {
    try {
        const response = await fetch('/api/config');
        if (response.ok) {
            const config = await response.json();
            
            // 填充表单
            document.getElementById('api_url').value = config.api_url || '';
            document.getElementById('api_key').value = config.api_key || '';
            document.getElementById('search_model_id').value = config.search_model_id || '';
            document.getElementById('analysis_model_id').value = config.analysis_model_id || '';
            document.getElementById('timeout').value = config.timeout || 180;
            document.getElementById('stream').checked = config.stream !== false;
            document.getElementById('filter_thinking').checked = config.filter_thinking !== false;
            document.getElementById('analysis_retry_count').value = config.analysis_retry_count || 1;
            document.getElementById('search_retry_count').value = config.search_retry_count || 0;
            document.getElementById('log_level').value = config.log_level || 'INFO';
            document.getElementById('max_query_plan').value = config.max_query_plan || 1;
            document.getElementById('http_api_key').value = config.http_api_key || 'xinchen';
            document.getElementById('admin_password').value = config.admin_password || 'xinchen';
            
            // 提示词：如果配置为空，显示默认值
            document.getElementById('system_prompt').value = config.system_prompt || DEFAULT_SYSTEM_PROMPT;
            document.getElementById('split_prompt').value = config.split_prompt || DEFAULT_SPLIT_PROMPT;
        }
    } catch (error) {
        console.log('配置文件不存在，使用默认值');
        // 使用默认值填充
        document.getElementById('timeout').value = 180;
        document.getElementById('system_prompt').value = DEFAULT_SYSTEM_PROMPT;
        document.getElementById('split_prompt').value = DEFAULT_SPLIT_PROMPT;
    }
}

// 保存配置
document.getElementById('configForm').addEventListener('submit', async (e) => {
    e.preventDefault();
    
    const submitBtn = e.target.querySelector('button[type="submit"]');
    submitBtn.disabled = true;
    submitBtn.textContent = '保存中...';
    
    const formData = new FormData(e.target);
    const systemPrompt = formData.get('system_prompt').trim();
    const splitPrompt = formData.get('split_prompt').trim();
    
    const config = {
        api_url: formData.get('api_url'),
        api_key: formData.get('api_key'),
        search_model_id: formData.get('search_model_id'),
        analysis_model_id: formData.get('analysis_model_id') || null,
        timeout: parseInt(formData.get('timeout')),
        stream: document.getElementById('stream').checked,
        filter_thinking: document.getElementById('filter_thinking').checked,
        analysis_retry_count: parseInt(formData.get('analysis_retry_count')),
        search_retry_count: parseInt(formData.get('search_retry_count')),
        log_level: formData.get('log_level'),
        max_query_plan: parseInt(formData.get('max_query_plan')),
        http_api_key: formData.get('http_api_key'),
        admin_password: formData.get('admin_password'),
        system_prompt: systemPrompt || null,
        split_prompt: splitPrompt || null,
    };
    
    try {
        const response = await fetch('/api/config', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(config),
        });
        
        const result = await response.json();
        
        if (response.ok) {
            showStatus('✅ ' + result.message);
            
            // 自动触发配置重新加载
            try {
                const restartResponse = await fetch('/api/restart', {
                    method: 'POST',
                });
                const restartResult = await restartResponse.json();
                if (restartResponse.ok) {
                    showStatus('✅ ' + restartResult.message);
                }
            } catch (error) {
                console.error('重新加载配置失败:', error);
            }
        } else {
            showStatus('❌ ' + result.error, true);
        }
    } catch (error) {
        showStatus('❌ 保存失败: ' + error.message, true);
    } finally {
        submitBtn.disabled = false;
        submitBtn.textContent = '保存配置';
    }
});

// 测试连接
document.getElementById('testBtn').addEventListener('click', async () => {
    const apiUrl = document.getElementById('api_url').value;
    const apiKey = document.getElementById('api_key').value;
    
    if (!apiUrl || !apiKey) {
        showStatus('❌ 请先填写 API URL 和 API Key', true);
        return;
    }
    
    const btn = document.getElementById('testBtn');
    btn.disabled = true;
    btn.textContent = '测试中...';
    
    try {
        const response = await fetch('/api/test', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ api_url: apiUrl, api_key: apiKey }),
        });
        
        const result = await response.json();
        
        if (response.ok) {
            showStatus('✅ 连接成功');
        } else {
            showStatus('❌ 连接失败: ' + result.error, true);
        }
    } catch (error) {
        showStatus('❌ 测试失败: ' + error.message, true);
    } finally {
        btn.disabled = false;
        btn.textContent = '测试连接';
    }
});

// 页面加载时先加载默认值，再加载配置
async function init() {
    await loadDefaults();
    await loadConfig();
}

init();
