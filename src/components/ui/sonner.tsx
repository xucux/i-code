"use client"

import { Toaster as Sonner } from "sonner"
import { useTheme } from "@/modules/theme/use-theme"

type ToasterProps = React.ComponentProps<typeof Sonner>

// Toast 通知组件：根据当前主题切换 sonner 的明暗模式，并使用 Font Awesome 图标
const Toaster = ({ ...props }: ToasterProps) => {
  const { theme } = useTheme()
  const sonnerTheme: ToasterProps["theme"] = theme.includes("dark") ? "dark" : "light"

  return (
    <Sonner
      theme={sonnerTheme}
      className="toaster group"
      position="top-center"
      offset={44}
      closeButton
      swipeDirections={['left', 'right']}
      icons={{
        success: <i className="fa-solid fa-circle-check h-4 w-4" />,
        info: <i className="fa-solid fa-circle-info h-4 w-4" />,
        warning: <i className="fa-solid fa-triangle-exclamation h-4 w-4" />,
        error: <i className="fa-solid fa-circle-xmark h-4 w-4" />,
        loading: <i className="fa-solid fa-circle-notch h-4 w-4 animate-spin" />,
        close: <i className="fa-solid fa-xmark h-3.5 w-3.5" />,
      }}
      toastOptions={{
        classNames: {
          toast:
            "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border-border group-[.toaster]:shadow-lg",
          description: "group-[.toast]:text-muted-foreground",
          closeButton:
            "group-[.toast]:border-border group-[.toast]:hover:bg-muted group-[.toast]:hover:text-foreground",
          actionButton:
            "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
          cancelButton:
            "group-[.toast]:bg-muted group-[.toast]:text-muted-foreground",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
