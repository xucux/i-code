import { forwardRef, useState } from 'react'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'

export interface SecretInputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'type'> {
  label?: string
}

/**
 * 密码/密钥输入框组件
 * 在右侧提供显示/隐藏密码的切换按钮，使用 Font Awesome 图标
 */
export const SecretInput = forwardRef<HTMLInputElement, SecretInputProps>(
  ({ className, ...props }, ref) => {
    // 当前是否明文显示密码
    const [visible, setVisible] = useState(false)

    return (
      // 相对定位容器，用于放置右侧眼睛图标按钮
      <div className="relative">
        <Input
          ref={ref}
          type={visible ? 'text' : 'password'}
          className={className}
          {...props}
        />
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="absolute right-0 top-0 h-full px-3"
          onClick={() => setVisible((v) => !v)}
        >
          {visible ? (
            <i className="fa-solid fa-eye-slash size-4" />
          ) : (
            <i className="fa-solid fa-eye size-4" />
          )}
        </Button>
      </div>
    )
  }
)

SecretInput.displayName = 'SecretInput'
