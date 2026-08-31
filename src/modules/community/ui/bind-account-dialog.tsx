/**
 * 匿名身份绑定账号弹窗（2026-08-31 鉴权迭代 D3）
 *
 * 匿名用户在此设置「唯一用户名 + 密码」：绑定后可在任意设备凭账号登录，
 * 原本机身份下的帖子 / 回复 / 积分原样保留（同 user_id）。
 * 成功后 Worker 吊销原匿名 token 并签发 account token。
 */

import { useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

export interface BindAccountDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** 提交绑定（用户名 + 密码） */
  onSubmit: (username: string, password: string) => void
  /** 提交执行中（禁用按钮防重复提交） */
  pending?: boolean
}

/** 前端本地规则校验（服务端权威，Worker 仍兜底校验）：字母开头 + 4~32 位英文/数字 */
const USERNAME_RE = /^[A-Za-z][A-Za-z0-9]{3,31}$/

export function BindAccountDialog({ open, onOpenChange, onSubmit, pending }: BindAccountDialogProps) {
  const { t } = useTranslation('community')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [error, setError] = useState<string | null>(null)

  /** 关闭时清空表单 */
  const handleOpenChange = (next: boolean) => {
    if (!next) {
      setUsername('')
      setPassword('')
      setConfirm('')
      setError(null)
    }
    onOpenChange(next)
  }

  /** 提交绑定（本地校验规则，Worker 仍兜底） */
  const submit = () => {
    if (!USERNAME_RE.test(username.trim())) {
      setError(t('auth.usernameRule'))
      return
    }
    if (password.length < 8) {
      setError(t('auth.passwordRule'))
      return
    }
    if (password !== confirm) {
      setError(t('auth.passwordMismatch'))
      return
    }
    setError(null)
    onSubmit(username.trim(), password)
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="w-96">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-sm">
            <i className="fa-solid fa-link text-primary size-3.5" />
            {t('auth.bindTitle')}
          </DialogTitle>
          <DialogDescription className="text-xs">{t('auth.bindDesc')}</DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <Input
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder={t('auth.usernamePlaceholder')}
            className="h-8 text-xs"
            autoComplete="off"
            disabled={pending}
          />
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={t('auth.passwordPlaceholder')}
            className="h-8 text-xs"
            autoComplete="new-password"
            disabled={pending}
          />
          <Input
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            placeholder={t('auth.confirmPasswordPlaceholder')}
            className="h-8 text-xs"
            autoComplete="new-password"
            disabled={pending}
            onKeyDown={(e) => {
              if (e.key === 'Enter') submit()
            }}
          />
          {error && (
            <p className="text-destructive text-[11px]">
              <i className="fa-solid fa-circle-exclamation mr-1 size-2.5" />
              {error}
            </p>
          )}
          <p className="text-muted-foreground text-[11px]">{t('auth.ruleHint')}</p>
        </div>

        <DialogFooter>
          <Button variant="outline" size="sm" className="h-7 text-xs" disabled={pending} onClick={() => handleOpenChange(false)}>
            {t('auth.cancel')}
          </Button>
          <Button size="sm" className="h-7 text-xs" disabled={pending} onClick={submit}>
            {pending && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
            {t('auth.bindSubmit')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}