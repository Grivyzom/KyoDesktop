import sys
import re

file_path = r'v:\Proyectos\Grivyzom Ideas\tauri-app\dioxus-chat\packages\ui\assets\styling\chat.css'

with open(file_path, 'r', encoding='utf-8') as f:
    content = f.read()

tokens = '''/* ==========================================================================
   KYO THEME TOKENS — Dark (default) / Light (:root.light)
   All component colors reference these variables for seamless theme switching.
   ========================================================================== */

:root {
  /* ─── Surface / Background ─── */
  --kyo-surface-0: #09090b;
  --kyo-surface-1: #18181b;
  --kyo-surface-2: #27272a;
  --kyo-surface-3: #3f3f46;

  /* ─── Sidebar ─── */
  --kyo-sidebar-bg: #0b0c11;
  --kyo-sidebar-border: #1a1d29;
  --kyo-sidebar-top: #0d0e14;
  --kyo-sidebar-dock: #0a0b10;
  --kyo-sidebar-input-bg: #10121a;
  --kyo-sidebar-input-border: #1c202e;
  --kyo-sidebar-hover: #161825;
  --kyo-sidebar-card-bg: #0f1118;
  --kyo-sidebar-expand-bg: #12141d;
  --kyo-sidebar-expand-border: #232738;
  --kyo-sidebar-expand-hover: #1b1e2c;
  --kyo-sidebar-agent-bg: #12131d;
  --kyo-sidebar-agent-border: #202436;
  --kyo-sidebar-agent-hover: #181a27;
  --kyo-sidebar-dropdown-bg: #12141f;
  --kyo-sidebar-dropdown-border: #252a3e;
  --kyo-sidebar-nav-hover: #171924;
  --kyo-sidebar-nav-active: #141620;
  --kyo-sidebar-icon-hover: #1a1d2b;
  --kyo-sidebar-icon-hover-border: #272c3f;
  --kyo-sidebar-item-hover: #1a1d2c;
  --kyo-sidebar-item-hover-border: #2b3149;
  --kyo-sidebar-input-focus-bg: #141722;
  --kyo-sidebar-badge-bg: #181b27;
  --kyo-sidebar-badge-border: #222637;
  --kyo-sidebar-ws-header-hover: #161824;
  --kyo-sidebar-chat-hover: #161825;
  --kyo-sidebar-chat-border-hover: #212538;
  --kyo-sidebar-dock-hover: #181b28;
  --kyo-sidebar-dock-hover-border: #262b3e;
  --kyo-sidebar-dock-active: #131520;
  --kyo-sidebar-ws-hover-border: #262b3c;
  --kyo-sidebar-agent-hover-border: #2d334d;
  --kyo-sidebar-nav-hover-border: #262a3d;

  /* ─── Foreground / Text ─── */
  --kyo-fg-primary: #f4f4f5;
  --kyo-fg-secondary: #e4e4e7;
  --kyo-fg-tertiary: #a1a1aa;
  --kyo-fg-muted: #71717a;
  --kyo-fg-subtle: #8b92a4;
  --kyo-fg-dimmed: #64748b;
  --kyo-fg-nav: #94a3b8;
  --kyo-fg-card: #cbd5e1;
  --kyo-fg-agent: #e2e8f0;
  --kyo-fg-input: #f1f5f9;
  --kyo-fg-placeholder: #4b5563;
  --kyo-fg-expand: #9ca3af;

  /* ─── Borders ─── */
  --kyo-border-default: #27272a;
  --kyo-border-subtle: rgba(39, 39, 42, 0.6);
  --kyo-border-medium: rgba(39, 39, 42, 0.8);
  --kyo-border-hover: #3f3f46;

  /* ─── Accent (indigo) ─── */
  --kyo-accent: #6366f1;
  --kyo-accent-deep: #4f46e5;
  --kyo-accent-soft: rgba(99, 102, 241, 0.15);
  --kyo-accent-border: rgba(99, 102, 241, 0.4);
  --kyo-accent-glow: rgba(99, 102, 241, 0.25);
  --kyo-accent-selected-fg: #e0e7ff;

  /* ─── Scrollbar ─── */
  --kyo-scrollbar-thumb: #27272a;
  --kyo-scrollbar-thumb-hover: #3f3f46;

  /* ─── Shadows ─── */
  --kyo-shadow-float: 0 4px 16px rgba(0, 0, 0, 0.5);
  --kyo-shadow-dropdown: 0 14px 32px rgba(0, 0, 0, 0.7);

  /* ─── Chat Area ─── */
  --kyo-chat-header-bg: rgba(255, 255, 255, 0.03);
  --kyo-chat-header-border: rgba(255, 255, 255, 0.08);
  --kyo-chat-nav-bg: rgba(255, 255, 255, 0.02);
  --kyo-chat-nav-border: rgba(255, 255, 255, 0.05);
}

:root.light {
  /* ─── Surface / Background ─── */
  --kyo-surface-0: #ffffff;
  --kyo-surface-1: #f8fafc;
  --kyo-surface-2: #f1f5f9;
  --kyo-surface-3: #e2e8f0;

  /* ─── Sidebar ─── */
  --kyo-sidebar-bg: #f8fafc;
  --kyo-sidebar-border: #e2e8f0;
  --kyo-sidebar-top: #f1f5f9;
  --kyo-sidebar-dock: #f8fafc;
  --kyo-sidebar-input-bg: #ffffff;
  --kyo-sidebar-input-border: #e2e8f0;
  --kyo-sidebar-hover: #f1f5f9;
  --kyo-sidebar-card-bg: #ffffff;
  --kyo-sidebar-expand-bg: #ffffff;
  --kyo-sidebar-expand-border: #e2e8f0;
  --kyo-sidebar-expand-hover: #f1f5f9;
  --kyo-sidebar-agent-bg: #ffffff;
  --kyo-sidebar-agent-border: #e2e8f0;
  --kyo-sidebar-agent-hover: #f1f5f9;
  --kyo-sidebar-dropdown-bg: #ffffff;
  --kyo-sidebar-dropdown-border: #e2e8f0;
  --kyo-sidebar-nav-hover: #f1f5f9;
  --kyo-sidebar-nav-active: #e2e8f0;
  --kyo-sidebar-icon-hover: #f1f5f9;
  --kyo-sidebar-icon-hover-border: #e2e8f0;
  --kyo-sidebar-item-hover: #f1f5f9;
  --kyo-sidebar-item-hover-border: #e2e8f0;
  --kyo-sidebar-input-focus-bg: #ffffff;
  --kyo-sidebar-badge-bg: #f1f5f9;
  --kyo-sidebar-badge-border: #e2e8f0;
  --kyo-sidebar-ws-header-hover: #f1f5f9;
  --kyo-sidebar-chat-hover: #f1f5f9;
  --kyo-sidebar-chat-border-hover: #e2e8f0;
  --kyo-sidebar-dock-hover: #f1f5f9;
  --kyo-sidebar-dock-hover-border: #e2e8f0;
  --kyo-sidebar-dock-active: #e2e8f0;
  --kyo-sidebar-ws-hover-border: #cbd5e1;
  --kyo-sidebar-agent-hover-border: #cbd5e1;
  --kyo-sidebar-nav-hover-border: #e2e8f0;

  /* ─── Foreground / Text ─── */
  --kyo-fg-primary: #0f172a;
  --kyo-fg-secondary: #1e293b;
  --kyo-fg-tertiary: #64748b;
  --kyo-fg-muted: #94a3b8;
  --kyo-fg-subtle: #64748b;
  --kyo-fg-dimmed: #94a3b8;
  --kyo-fg-nav: #475569;
  --kyo-fg-card: #1e293b;
  --kyo-fg-agent: #1e293b;
  --kyo-fg-input: #0f172a;
  --kyo-fg-placeholder: #94a3b8;
  --kyo-fg-expand: #64748b;

  /* ─── Borders ─── */
  --kyo-border-default: #e2e8f0;
  --kyo-border-subtle: rgba(226, 232, 240, 0.8);
  --kyo-border-medium: rgba(226, 232, 240, 0.9);
  --kyo-border-hover: #cbd5e1;

  /* ─── Accent (indigo) ─── */
  --kyo-accent: #4f46e5;
  --kyo-accent-deep: #4338ca;
  --kyo-accent-soft: rgba(79, 70, 229, 0.08);
  --kyo-accent-border: rgba(79, 70, 229, 0.3);
  --kyo-accent-glow: rgba(79, 70, 229, 0.15);
  --kyo-accent-selected-fg: #312e81;

  /* ─── Scrollbar ─── */
  --kyo-scrollbar-thumb: #cbd5e1;
  --kyo-scrollbar-thumb-hover: #94a3b8;

  /* ─── Shadows ─── */
  --kyo-shadow-float: 0 4px 16px rgba(0, 0, 0, 0.08);
  --kyo-shadow-dropdown: 0 14px 32px rgba(0, 0, 0, 0.12);

  /* ─── Chat Area ─── */
  --kyo-chat-header-bg: rgba(0, 0, 0, 0.02);
  --kyo-chat-header-border: rgba(0, 0, 0, 0.08);
  --kyo-chat-nav-bg: rgba(0, 0, 0, 0.02);
  --kyo-chat-nav-border: rgba(0, 0, 0, 0.06);
}

'''
content = content.replace('/* Universal Reset & Box Model */', tokens + '/* Universal Reset & Box Model */')

blocks = [
    ('html, body', 'background-color: #09090b;', 'background-color: var(--kyo-surface-0);'),
    ('html, body', 'color: #f4f4f5;', 'color: var(--kyo-fg-primary);'),

    ('::-webkit-scrollbar-thumb', 'background: #27272a;', 'background: var(--kyo-scrollbar-thumb);'),
    ('::-webkit-scrollbar-thumb:hover', 'background: #3f3f46;', 'background: var(--kyo-scrollbar-thumb-hover);'),

    ('.sidebar-container', 'background-color: #0b0c11;', 'background-color: var(--kyo-sidebar-bg);'),
    ('.sidebar-container', 'border-right: 1px solid #1a1d29;', 'border-right: 1px solid var(--kyo-sidebar-border);'),

    ('.sidebar-floating-expand-btn', 'background-color: #12141d;', 'background-color: var(--kyo-sidebar-expand-bg);'),
    ('.sidebar-floating-expand-btn', 'border: 1px solid #232738;', 'border: 1px solid var(--kyo-sidebar-expand-border);'),
    ('.sidebar-floating-expand-btn', 'color: #9ca3af;', 'color: var(--kyo-fg-expand);'),
    ('.sidebar-floating-expand-btn', 'box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);', 'box-shadow: var(--kyo-shadow-float);'),

    ('.sidebar-floating-expand-btn:hover', 'background-color: #1b1e2c;', 'background-color: var(--kyo-sidebar-expand-hover);'),
    ('.sidebar-floating-expand-btn:hover', 'border-color: #6366f1;', 'border-color: var(--kyo-accent);'),

    ('.sidebar-top-section', 'border-bottom: 1px solid #1a1d29;', 'border-bottom: 1px solid var(--kyo-sidebar-border);'),
    ('.sidebar-top-section', 'background-color: #0d0e14;', 'background-color: var(--kyo-sidebar-top);'),

    ('.sidebar-header-title', 'color: #8b92a4;', 'color: var(--kyo-fg-subtle);'),

    ('.sidebar-icon-btn', 'color: #8b92a4;', 'color: var(--kyo-fg-subtle);'),
    ('.sidebar-icon-btn:hover', 'background-color: #1a1d2b;', 'background-color: var(--kyo-sidebar-icon-hover);'),
    ('.sidebar-icon-btn:hover', 'border-color: #272c3f;', 'border-color: var(--kyo-sidebar-icon-hover-border);'),
    ('.sidebar-icon-btn:hover', 'color: #f1f5f9;', 'color: var(--kyo-fg-input);'),

    ('.sidebar-btn-nav', 'background-color: rgba(255, 255, 255, 0.02);', 'background-color: var(--kyo-chat-nav-bg);'),
    ('.sidebar-btn-nav', 'color: #94a3b8;', 'color: var(--kyo-fg-nav);'),
    ('.sidebar-btn-nav', 'border: 1px solid rgba(255, 255, 255, 0.05);', 'border: 1px solid var(--kyo-chat-nav-border);'),

    ('.sidebar-btn-nav:hover', 'background-color: #171924;', 'background-color: var(--kyo-sidebar-nav-hover);'),
    ('.sidebar-btn-nav:hover', 'border-color: #262a3d;', 'border-color: var(--kyo-sidebar-nav-hover-border);'),

    ('.sidebar-btn-nav:active', 'background-color: #141620;', 'background-color: var(--kyo-sidebar-nav-active);'),

    ('.sidebar-agent-button', 'background-color: #12131d;', 'background-color: var(--kyo-sidebar-agent-bg);'),
    ('.sidebar-agent-button', 'color: #e2e8f0;', 'color: var(--kyo-fg-agent);'),
    ('.sidebar-agent-button', 'border: 1px solid #202436;', 'border: 1px solid var(--kyo-sidebar-agent-border);'),

    ('.sidebar-agent-button:hover', 'background-color: #181a27;', 'background-color: var(--kyo-sidebar-agent-hover);'),
    ('.sidebar-agent-button:hover', 'border-color: #2d334d;', 'border-color: var(--kyo-sidebar-agent-hover-border);'),

    ('.sidebar-agent-dropdown', 'background-color: #12141f;', 'background-color: var(--kyo-sidebar-dropdown-bg);'),
    ('.sidebar-agent-dropdown', 'border: 1px solid #252a3e;', 'border: 1px solid var(--kyo-sidebar-dropdown-border);'),
    ('.sidebar-agent-dropdown', 'box-shadow: 0 14px 32px rgba(0, 0, 0, 0.7);', 'box-shadow: var(--kyo-shadow-dropdown);'),

    ('.sidebar-agent-item', 'color: #94a3b8;', 'color: var(--kyo-fg-nav);'),
    ('.sidebar-agent-item:hover', 'background-color: #1a1d2c;', 'background-color: var(--kyo-sidebar-item-hover);'),
    ('.sidebar-agent-item:hover', 'border-color: #2b3149;', 'border-color: var(--kyo-sidebar-item-hover-border);'),

    ('.sidebar-agent-item.selected', 'background-color: rgba(99, 102, 241, 0.15);', 'background-color: var(--kyo-accent-soft);'),
    ('.sidebar-agent-item.selected', 'border-color: rgba(99, 102, 241, 0.4);', 'border-color: var(--kyo-accent-border);'),
    ('.sidebar-agent-item.selected', 'color: #e0e7ff;', 'color: var(--kyo-accent-selected-fg);'),

    ('.sidebar-search-input', 'background-color: #10121a;', 'background-color: var(--kyo-sidebar-input-bg);'),
    ('.sidebar-search-input', 'border: 1px solid #1c202e;', 'border: 1px solid var(--kyo-sidebar-input-border);'),
    ('.sidebar-search-input', 'color: #f1f5f9;', 'color: var(--kyo-fg-input);'),

    ('.sidebar-search-input::placeholder', 'color: #4b5563;', 'color: var(--kyo-fg-placeholder);'),

    ('.sidebar-search-input:focus', 'background-color: #141722;', 'background-color: var(--kyo-sidebar-input-focus-bg);'),
    ('.sidebar-search-input:focus', 'border-color: #6366f1;', 'border-color: var(--kyo-accent);'),
    ('.sidebar-search-input:focus', 'box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.25);', 'box-shadow: 0 0 0 2px var(--kyo-accent-glow);'),

    ('.sidebar-section-title', 'color: #64748b;', 'color: var(--kyo-fg-dimmed);'),

    ('.workspace-card', 'background-color: #0f1118;', 'background-color: var(--kyo-sidebar-card-bg);'),
    ('.workspace-card', 'border: 1px solid #1a1d29;', 'border: 1px solid var(--kyo-sidebar-border);'),

    ('.workspace-card:hover', 'border-color: #262b3c;', 'border-color: var(--kyo-sidebar-ws-hover-border);'),

    ('.workspace-header-btn', 'color: #cbd5e1;', 'color: var(--kyo-fg-card);'),
    ('.workspace-header-btn:hover', 'background-color: #161824;', 'background-color: var(--kyo-sidebar-ws-header-hover);'),

    ('.workspace-badge', 'background-color: #181b27;', 'background-color: var(--kyo-sidebar-badge-bg);'),
    ('.workspace-badge', 'border: 1px solid #222637;', 'border: 1px solid var(--kyo-sidebar-badge-border);'),
    ('.workspace-badge', 'color: #8b92a4;', 'color: var(--kyo-fg-subtle);'),

    ('.chat-sublist', 'border-top: 1px solid #181b27;', 'border-top: 1px solid var(--kyo-sidebar-badge-bg);'),

    ('.chat-item-btn', 'color: #8b92a4;', 'color: var(--kyo-fg-subtle);'),
    ('.chat-item-btn:hover', 'background-color: #161825;', 'background-color: var(--kyo-sidebar-chat-hover);'),
    ('.chat-item-btn:hover', 'color: #f1f5f9;', 'color: var(--kyo-fg-input);'),
    ('.chat-item-btn:hover', 'border-color: #212538;', 'border-color: var(--kyo-sidebar-chat-border-hover);'),

    ('.chat-item-btn.active', 'background-color: rgba(99, 102, 241, 0.15);', 'background-color: var(--kyo-accent-soft);'),
    ('.chat-item-btn.active', 'color: #f1f5f9;', 'color: var(--kyo-fg-input);'),
    ('.chat-item-btn.active', 'border-color: rgba(99, 102, 241, 0.3);', 'border-color: var(--kyo-accent-border);'),
    ('.chat-item-btn.active', 'border-left: 2px solid #6366f1;', 'border-left: 2px solid var(--kyo-accent);'),

    ('.sidebar-dock', 'border-top: 1px solid #1a1d29;', 'border-top: 1px solid var(--kyo-sidebar-border);'),
    ('.sidebar-dock', 'background-color: #0a0b10;', 'background-color: var(--kyo-sidebar-dock);'),

    ('.sidebar-dock-btn', 'color: #8b92a4;', 'color: var(--kyo-fg-subtle);'),
    ('.sidebar-dock-btn:hover', 'background-color: #181b28;', 'background-color: var(--kyo-sidebar-dock-hover);'),
    ('.sidebar-dock-btn:hover', 'border-color: #262b3e;', 'border-color: var(--kyo-sidebar-dock-hover-border);'),

    ('.sidebar-dock-btn:active', 'background-color: #131520;', 'background-color: var(--kyo-sidebar-dock-active);'),

    ('.chat-header-btn', 'color: #94a3b8;', 'color: var(--kyo-fg-nav);'),
    ('.chat-header-btn', 'background-color: rgba(255, 255, 255, 0.03);', 'background-color: var(--kyo-chat-header-bg);'),
    ('.chat-header-btn', 'border: 1px solid rgba(255, 255, 255, 0.08);', 'border: 1px solid var(--kyo-chat-header-border);'),
]

for block_name, old_val, new_val in blocks:
    pattern = re.escape(block_name) + r'\s*(::[a-z\-]+)*\s*[^\{]*\{([^}]+)\}'
    def block_replacer(match):
        inner = match.group(2) if match.group(2) else match.group(1) # just in case of pseudos
        new_inner = inner.replace(old_val, new_val)
        return match.group(0).replace(inner, new_inner)
    
    # We will use re.sub, but only replace the first occurrence or all occurrences of that block
    content = re.sub(pattern, block_replacer, content)

with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content)

print('Done')
