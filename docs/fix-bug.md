 修复 Radix ScrollArea Viewport 内部 display:table 包裹层问题

```css
 /* 在你的全局样式或组件样式中 */
[data-radix-scroll-area-viewport] > div {
  display: block !important;
}
```