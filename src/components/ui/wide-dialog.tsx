"use client"

import * as React from "react"

import { cn } from "@/lib/utils"
import { DialogContent } from "@/components/ui/dialog"

type WideDialogContentProps = React.ComponentPropsWithoutRef<typeof DialogContent>

/**
 * 超宽弹窗内容
 *
 * 基于 shadcn/ui DialogContent，将最大宽度扩展到 `max-w-4xl`，
 * 用于需要左右分栏（如表单 + 穿梭框）的复杂新建/编辑场景。
 */
export const WideDialogContent = React.forwardRef<
  React.ElementRef<typeof DialogContent>,
  WideDialogContentProps
>(({ className, children, ...props }, ref) => (
  <DialogContent
    ref={ref}
    className={cn("max-w-4xl", className)}
    {...props}
  >
    {children}
  </DialogContent>
))
WideDialogContent.displayName = "WideDialogContent"
