/**
 * Mouse-helper: injects a visible cursor + click ripple into the page.
 * Used by Playwright recordVideo for demo GIF phases and integration tests.
 *
 * Reused in:
 *   - Track B (level 2 integration tests)
 *   - Track C (demo GIF phase 1 + phase 3)
 *
 * Source: research § "Playwright recording specifics"
 */
export function installMouseHelper(): string {
  return `
    // Install mouse helper — shows cursor and click ripples
    (() => {
      const box = document.createElement('playwright-mouse-pointer');
      const style = document.createElement('style');
      style.textContent = \`
        playwright-mouse-pointer {
          pointer-events: none;
          position: fixed;
          top: 0;
          z-index: 10000;
          left: 0;
          width: 20px;
          height: 20px;
          background: rgba(0, 0, 0, 0.4);
          border: 1px solid white;
          border-radius: 10px;
          margin: -10px 0 0 -10px;
          padding: 0;
          transition: background 0.2s, border-color 0.2s, transform 0.2s;
        }
        playwright-mouse-pointer.button-1 {
          transition: none;
          background: rgba(0, 0, 0, 0.9);
          transform: scale(1.3);
        }
        playwright-mouse-pointer.button-2 {
          transition: none;
          border-color: rgba(0, 0, 255, 0.9);
        }
        playwright-mouse-pointer.button-3 {
          transition: none;
          border-color: rgba(0, 255, 0, 0.9);
        }
        playwright-mouse-pointer.button-4 {
          transition: none;
          border-color: rgba(255, 255, 0, 0.9);
        }
        playwright-mouse-pointer.button-5 {
          transition: none;
          border-color: rgba(255, 0, 255, 0.9);
        }
      \`;
      document.head.appendChild(style);
      document.body.appendChild(box);

      document.addEventListener('mousemove', (event) => {
        box.style.left = event.pageX + 'px';
        box.style.top = event.pageY + 'px';
      }, true);

      document.addEventListener('mousedown', (event) => {
        box.classList.add('button-' + event.which);

        // Ripple effect
        const ripple = document.createElement('playwright-mouse-ripple');
        ripple.style.cssText = \`
          pointer-events: none;
          position: fixed;
          z-index: 9999;
          left: \${event.clientX}px;
          top: \${event.clientY}px;
          width: 0;
          height: 0;
          border-radius: 50%;
          border: 2px solid rgba(0, 0, 0, 0.4);
          transform: translate(-50%, -50%);
          animation: playwright-ripple 0.6s ease-out forwards;
        \`;
        document.body.appendChild(ripple);
        setTimeout(() => ripple.remove(), 700);
      }, true);

      document.addEventListener('mouseup', (event) => {
        box.classList.remove('button-' + event.which);
      }, true);

      // Add ripple animation
      const animStyle = document.createElement('style');
      animStyle.textContent = \`
        @keyframes playwright-ripple {
          to {
            width: 40px;
            height: 40px;
            border-width: 0;
            opacity: 0;
          }
        }
      \`;
      document.head.appendChild(animStyle);
    })();
  `;
}
