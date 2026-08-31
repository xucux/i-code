/**
 * 社区登录 / 进入页（§8.2 迭代，2026-08-31 鉴权迭代）
 *
 * 原「Switch 开启」卡片改为双模式进入卡：
 * - 匿名进入（本机机器码身份，无需密码）；
 * - 用户名密码登录 / 注册（账号跨设备，注册=新建独立身份）。
 *
 * 未开启社区（enabled=false）时同样展示本卡；三种入口成功后由父级置 enabled 并进入。
 */

import { useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

export interface CommunityGateProps {
  /** 请求执行中（禁用按钮防重复提交） */
  pending: boolean
  /** 匿名进入（未开启时会先开启门禁并换取匿名 token） */
  onAnonymous: () => void
  /** 账号登录 */
  onLogin: (username: string, password: string) => void
  /** 注册账号 */
  onRegister: (username: string, password: string) => void
}

/** 前端本地规则校验（服务端权威，Worker 仍兜底校验）：字母开头 + 4~32 位英文/数字 */
const USERNAME_RE = /^[A-Za-z][A-Za-z0-9]{3,31}$/

export function CommunityGate({ pending, onAnonymous, onLogin, onRegister }: CommunityGateProps) {
  const { t } = useTranslation('community')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)

  /** 本地校验用户名 / 密码规则，返回错误文案；null = 通过 */
  const validate = (): string | null => {
    if (!USERNAME_RE.test(username.trim())) {
      return t('auth.usernameRule')
    }
    if (password.length < 8) {
      return t('auth.passwordRule')
    }
    return null
  }

  /** 登录 / 注册统一提交入口 */
  const submit = (mode: 'login' | 'register') => {
    const err = validate()
    if (err) {
      setError(err)
      return
    }
    setError(null)
    if (mode === 'login') onLogin(username.trim(), password)
    else onRegister(username.trim(), password)
  }

  return (
    // 整页模糊背景：内容区实际渲染在卡片下方，保证模糊效果可见
    <div className="relative h-full w-full overflow-hidden">
      {/* 占位内容：被模糊的示意层（帖子列表轮廓） */}
      <div className="absolute inset-0 space-y-3 p-6 opacity-60" aria-hidden>
        <div className="h-8 w-1/3 rounded-md bg-muted" />
        {[80, 62, 70, 55, 66].map((w, i) => (
          <div key={i} className="h-20 rounded-md border bg-card" style={{ width: `${w}%` }} />
        ))}
      </div>

      {/* 居中说明卡片 */}
      <div className="absolute inset-0 flex items-center justify-center backdrop-blur-lg">
        <Card className="w-96">
          <CardContent className="space-y-4 p-6">
            <div className="flex items-center gap-2">
              <i className="fa-solid fa-users text-primary size-4" />
              <h2 className="text-base font-semibold">{t('gate.title')}</h2>
            </div>

            <p className="text-muted-foreground text-xs leading-relaxed">{t('auth.desc')}</p>

            {/* 匿名进入：本机身份，无需密码 */}
            <Button className="h-8 w-full text-xs" disabled={pending} onClick={onAnonymous}>
              {pending && <i className="fa-solid fa-spinner fa-spin mr-1.5 size-3" />}
              <i className="fa-solid fa-user-secret mr-1.5 size-3" />
              {t('auth.anonymousEnter')}
            </Button>

            {/* 匿名身份隐私提示：紧随匿名按钮，强调本机绑定语义 */}
            <p className="text-muted-foreground flex items-start gap-1.5 text-[11px] leading-relaxed">
              <i className="fa-solid fa-shield-halved mt-0.5 size-3 shrink-0" />
              <span>{t('gate.privacy')}</span>
            </p>

            <div className="text-muted-foreground flex items-center gap-2 text-[11px]">
              <span className="bg-muted h-px flex-1" />
              {t('auth.orAccount')}
              <span className="bg-muted h-px flex-1" />
            </div>

            {/* 账号表单：登录 / 注册 */}
            <div className="space-y-2">
              <Input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder={t('auth.usernamePlaceholder')}
                className="h-8 text-xs"
                autoComplete="username"
                disabled={pending}
              />
              <Input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t('auth.passwordPlaceholder')}
                className="h-8 text-xs"
                autoComplete="current-password"
                disabled={pending}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') submit('login')
                }}
              />
              {error && (
                <p className="text-destructive text-[11px]">
                  <i className="fa-solid fa-circle-exclamation mr-1 size-2.5" />
                  {error}
                </p>
              )}
              <div className="flex gap-2">
                <Button
                  variant="outline"
                  className="h-7 flex-1 text-xs"
                  disabled={pending}
                  onClick={() => submit('login')}
                >
                  {t('auth.login')}
                </Button>
                <Button className="h-7 flex-1 text-xs" disabled={pending} onClick={() => submit('register')}>
                  {t('auth.register')}
                </Button>
              </div>
              <p className="text-muted-foreground text-[11px]">{t('auth.ruleHint')}</p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}