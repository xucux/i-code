import { useState, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'

interface FileDropZoneProps {
  accept?: string
  maxFiles?: number
  onFilesChange?: (files: File[]) => void
}

/**
 * 文件拖拽上传组件
 * 支持点击选择文件与拖拽文件进入区域两种方式
 */
export function FileDropZone({
  accept,
  maxFiles = 5,
  onFilesChange,
}: FileDropZoneProps) {
  // 已选文件列表
  const [files, setFiles] = useState<File[]>([])
  // 拖拽悬停状态，用于切换区域视觉样式
  const [isDragging, setIsDragging] = useState(false)

  // 统一更新文件状态并通知外部回调
  const updateFiles = useCallback(
    (next: File[]) => {
      setFiles(next)
      onFilesChange?.(next)
    },
    [onFilesChange]
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setIsDragging(false)
  }, [])

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setIsDragging(false)
      const dropped = Array.from(e.dataTransfer.files).slice(0, maxFiles)
      updateFiles(dropped)
    },
    [maxFiles, updateFiles]
  )

  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const selected = Array.from(e.target.files ?? []).slice(0, maxFiles)
      updateFiles(selected)
    },
    [maxFiles, updateFiles]
  )

  const removeFile = useCallback(
    (index: number) => {
      const next = files.filter((_, i) => i !== index)
      updateFiles(next)
    },
    [files, updateFiles]
  )

  return (
    <div className="space-y-3">
      {/* 拖拽区域：根据 isDragging 状态切换高亮边框 */}
      <div
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={cn(
          'relative flex cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed bg-muted/30 px-4 py-8 transition-colors',
          isDragging
            ? 'border-primary bg-primary/5'
            : 'border-border hover:border-muted-foreground/50'
        )}
      >
        {/* 透明文件输入，覆盖整个拖拽区域以支持点击选择 */}
        <input
          type="file"
          accept={accept}
          multiple={maxFiles > 1}
          onChange={handleInputChange}
          className="absolute inset-0 cursor-pointer opacity-0"
        />
        <i className="fa-solid fa-cloud-arrow-up mb-2 size-8 text-muted-foreground" />
        <p className="text-sm font-medium">点击或拖拽文件到此处</p>
        <p className="text-xs text-muted-foreground">
          最多 {maxFiles} 个文件{accept ? `，支持 ${accept}` : ''}
        </p>
      </div>

      {files.length > 0 && (
        <ul className="space-y-2">
          {files.map((file, index) => (
            <li
              key={`${file.name}-${index}`}
              className="flex items-center justify-between rounded-md border bg-background px-3 py-2 text-xs"
            >
              <span className="truncate">{file.name}</span>
              <Button
                variant="ghost"
                size="icon"
                className="size-6"
                onClick={() => removeFile(index)}
              >
                <i className="fa-solid fa-xmark size-3.5" />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
