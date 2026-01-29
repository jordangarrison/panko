/**
 * Agent Replay - Keyboard Navigation
 *
 * Provides vim-style navigation between session blocks:
 * - j/Down: Next block
 * - k/Up: Previous block
 * - Enter/Space: Expand/collapse details in tool blocks
 * - gg: Go to first block
 * - G: Go to last block
 * - ?: Show keyboard shortcuts help
 * - Escape: Close help overlay
 */

(function() {
    'use strict';

    // State for multi-key commands (like 'gg')
    let pendingKey = null;
    let pendingKeyTimeout = null;

    // Get all navigable blocks
    function getBlocks() {
        return Array.from(document.querySelectorAll('.block[tabindex]'));
    }

    // Get currently focused block index
    function getCurrentIndex(blocks) {
        const focused = document.activeElement;
        return blocks.indexOf(focused);
    }

    // Focus a block and scroll it into view
    function focusBlock(block) {
        if (block) {
            block.focus();
            block.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
    }

    // Navigate to next block
    function nextBlock() {
        const blocks = getBlocks();
        const current = getCurrentIndex(blocks);
        const next = current < 0 ? 0 : Math.min(current + 1, blocks.length - 1);
        focusBlock(blocks[next]);
    }

    // Navigate to previous block
    function prevBlock() {
        const blocks = getBlocks();
        const current = getCurrentIndex(blocks);
        const prev = current < 0 ? 0 : Math.max(current - 1, 0);
        focusBlock(blocks[prev]);
    }

    // Navigate to first block
    function firstBlock() {
        const blocks = getBlocks();
        if (blocks.length > 0) {
            focusBlock(blocks[0]);
        }
    }

    // Navigate to last block
    function lastBlock() {
        const blocks = getBlocks();
        if (blocks.length > 0) {
            focusBlock(blocks[blocks.length - 1]);
        }
    }

    // Toggle expand/collapse on tool block details
    function toggleExpand() {
        const focused = document.activeElement;
        if (focused && focused.classList.contains('block')) {
            const details = focused.querySelector('details');
            if (details) {
                details.open = !details.open;
            }
        }
    }

    // Show help overlay
    function showHelp() {
        const overlay = document.getElementById('help-overlay');
        if (overlay) {
            overlay.classList.add('visible');
            overlay.setAttribute('aria-hidden', 'false');
            // Focus the close button for accessibility
            const closeBtn = overlay.querySelector('.help-close');
            if (closeBtn) {
                closeBtn.focus();
            }
        }
    }

    // Hide help overlay
    function hideHelp() {
        const overlay = document.getElementById('help-overlay');
        if (overlay) {
            overlay.classList.remove('visible');
            overlay.setAttribute('aria-hidden', 'true');
        }
    }

    // Check if help overlay is visible
    function isHelpVisible() {
        const overlay = document.getElementById('help-overlay');
        return overlay && overlay.classList.contains('visible');
    }

    // Clear pending key state
    function clearPendingKey() {
        pendingKey = null;
        if (pendingKeyTimeout) {
            clearTimeout(pendingKeyTimeout);
            pendingKeyTimeout = null;
        }
    }

    // Handle keyboard events
    function handleKeyDown(event) {
        // Don't capture keys when typing in an input
        if (event.target.tagName === 'INPUT' ||
            event.target.tagName === 'TEXTAREA' ||
            event.target.isContentEditable) {
            return;
        }

        const key = event.key;

        // Always handle Escape
        if (key === 'Escape') {
            if (isHelpVisible()) {
                hideHelp();
                event.preventDefault();
            }
            clearPendingKey();
            return;
        }

        // If help is visible, don't process other keys
        if (isHelpVisible()) {
            return;
        }

        // Handle multi-key sequences
        if (pendingKey === 'g') {
            clearPendingKey();
            if (key === 'g') {
                firstBlock();
                event.preventDefault();
            }
            return;
        }

        // Handle single keys
        switch (key) {
            case 'j':
            case 'ArrowDown':
                nextBlock();
                event.preventDefault();
                break;

            case 'k':
            case 'ArrowUp':
                prevBlock();
                event.preventDefault();
                break;

            case 'Enter':
            case ' ':
                toggleExpand();
                event.preventDefault();
                break;

            case 'g':
                // Start multi-key sequence
                pendingKey = 'g';
                pendingKeyTimeout = setTimeout(clearPendingKey, 1000);
                event.preventDefault();
                break;

            case 'G':
                lastBlock();
                event.preventDefault();
                break;

            case '?':
                showHelp();
                event.preventDefault();
                break;
        }
    }

    // Initialize
    function init() {
        // Keyboard event listener
        document.addEventListener('keydown', handleKeyDown);

        // Help button click
        const helpHint = document.querySelector('.help-hint');
        if (helpHint) {
            helpHint.addEventListener('click', showHelp);
        }

        // Help close button click
        const helpClose = document.querySelector('.help-close');
        if (helpClose) {
            helpClose.addEventListener('click', hideHelp);
        }

        // Close help on overlay background click
        const helpOverlay = document.getElementById('help-overlay');
        if (helpOverlay) {
            helpOverlay.addEventListener('click', function(event) {
                if (event.target === helpOverlay) {
                    hideHelp();
                }
            });
        }

        // Focus first block on page load (after a small delay for rendering)
        setTimeout(function() {
            const blocks = getBlocks();
            if (blocks.length > 0 && !document.activeElement.classList.contains('block')) {
                // Only auto-focus if nothing else is focused
                blocks[0].focus();
            }
        }, 100);
    }

    // Run init when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
