/**
 * 供应商扩展模板变量列表编辑器
 *
 * 管理 key/value/isSecret/label 的列表，运行时注入额度脚本为只读常量。
 * 紧凑布局适配 900×700 窗口，使用 grid-cols-[120px_1fr_120px_28px] 排列。
 */

import { useState } from 'react'
import { useTranslation } from '@/modules/i18n/use-translation'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import type { ProviderScriptVariable } from '@/modules/ai-gateway/types'
import { SCRIPT_VARIABLE_RESERVED_NAMES } from '@/modules/ai-gateway/types'

interface ScriptVariablesEditorProps {
  variables: ProviderScriptVariable[]
  onChange: (variables: ProviderScriptVariable[]) => void
  isEdit: boolean
}

const KEY_PATTERN = /^[a-zA-Z_][a-zA-Z0-9_]*$/

export function ScriptVariablesEditor({ variables, onChange, isEdit }: ScriptVariablesEditorProps) {
  const { t } = useTranslation()

  // 校验错误状态
  const [errors, setErrors] = useState<Record<number, { key?: string; value?: string }>>({})

  const addItem = () => {
    const newItem: ProviderScriptVariable = { key: '', value: '', isSecret: false }
    onChange([...variables, newItem])
  }

  const removeItem = (index: number) => {
    const next = variables.filter((_, i) => i !== index)
    onChange(next)
    // 清除对应错误
    const nextErrors = { ...errors }
    delete nextErrors[index]
    // 重新编号后续错误
    const reindexed: Record<number, { key?: string; value?: string }> = {}
    for (const [k, v] of Object.entries(nextErrors)) {
      const ki = Number(k)
      reindexed[ki > index ? ki - 1 : ki] = v
    }
    setErrors(reindexed)
  }

  const updateItem = (index: number, field: keyof ProviderScriptVariable, value: string | boolean) => {
    const next = variables.map((v, i) => i === index ? { ...v, [field]: value } : v)
    onChange(next)
    validateItem(index, next[index])
  }

  const validateItem = (index: number, item: ProviderScriptVariable) => {
    const err: { key?: string; value?: string } = {}
    // key 格式
    if (item.key && !KEY_PATTERN.test(item.key)) {
      err.key = t('aiGateway.providerForm.scriptVariables.keyInvalid')
    }
    // 保留名
    if (SCRIPT_VARIABLE_RESERVED_NAMES.includes(item.key)) {
      err.key = t('aiGateway.providerForm.scriptVariables.keyReserved')
    }
    // 重复名
    const duplicate = variables.some((v, i) => i !== index && v.key.toLowerCase() === item.key.toLowerCase())
    if (duplicate && item.key) {
      err.key = t('aiGateway.providerForm.scriptVariables.keyDuplicate')
    }
    setErrors((prev) => {
      const next = { ...prev }
      if (err.key || err.value) {
        next[index] = err
      } else {
        delete next[index]
      }
      return next
    })
  }

  return (
    <div className="space-y-3">
      <div className="text-xs font-medium">{t('aiGateway.providerForm.scriptVariables.title')}</div>
      <div className="text-xs text-muted-foreground">
        {t('aiGateway.providerForm.scriptVariables.description')}
      </div>

      {variables.length > 0 && (
        <div className="space-y-2">
          {variables.map((item, index) => (
            <div key={index} className="grid grid-cols-[120px_1fr_120px_28px] gap-2 items-start">
              {/* Key */}
              <div>
                <Input
                  value={item.key}
                  onChange={(e) => updateItem(index, 'key', e.target.value)}
                  className={`h-7 text-xs ${errors[index]?.key ? 'border-red-500' : ''}`}
                  placeholder={t('aiGateway.providerForm.scriptVariables.keyPlaceholder')}
                  onBlur={() => validateItem(index, item)}
                />
                {errors[index]?.key && (
                  <div className="text-[10px] text-red-500 mt-0.5 leading-tight">{errors[index]?.key}</div>
                )}
              </div>

              {/* Value */}
              <div>
                {item.isSecret ? (
                  <TooltipProvider>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Input
                          type="password"
                          value={isEdit && item.value.startsWith('$SECRET:') ? '••••••••' : item.value}
                          onChange={(e) => {
                            // 编辑模式：输入新值替换旧引用
                            const newVal = e.target.value
                            updateItem(index, 'value', newVal)
                          }}
                          onFocus={(e) => {
                            // 编辑模式且显示占位时：清空以允许输入新值
                            if (isEdit && item.value.startsWith('$SECRET:')) {
                              e.target.value = ''
                            }
                          }}
                          className="h-7 text-xs"
                          placeholder={isEdit && item.value.startsWith('$SECRET:')
                            ? t('aiGateway.providerForm.scriptVariables.secretPlaceholderEdit')
                            : t('aiGateway.providerForm.scriptVariables.valuePlaceholder')}
                        />
                      </TooltipTrigger>
                      {isEdit && item.value.startsWith('$SECRET:') && (
                        <TooltipContent side="top" className="text-xs">
                          {t('aiGateway.providerForm.scriptVariables.secretTooltipEdit')}
                        </TooltipContent>
                      )}
                    </Tooltip>
                  </TooltipProvider>
                ) : (
                  <Input
                    value={item.value}
                    onChange={(e) => updateItem(index, 'value', e.target.value)}
                    className="h-7 text-xs"
                    placeholder={t('aiGateway.providerForm.scriptVariables.valuePlaceholder')}
                  />
                )}
              </div>

              {/* Label + isSecret */}
              <div className="flex items-center gap-1">
                <Input
                  value={item.label ?? ''}
                  onChange={(e) => updateItem(index, 'label', e.target.value)}
                  className="h-7 text-xs w-[68px]"
                  placeholder={t('aiGateway.providerForm.scriptVariables.labelPlaceholder')}
                />
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div className="flex items-center">
                        <Switch
                          checked={item.isSecret ?? false}
                          onCheckedChange={(v) => {
                            // 切换到非敏感时：如果值是 $SECRET 引用，清空它
                            if (!v && item.value.startsWith('$SECRET:')) {
                              updateItem(index, 'isSecret', v)
                              // 后端会在下一次保存时处理引用清理
                            } else {
                              updateItem(index, 'isSecret', v)
                            }
                          }}
                          className="scale-75"
                        />
                      </div>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="text-xs">
                      {t('aiGateway.providerForm.scriptVariables.isSecret')}
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>

              {/* Delete */}
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 shrink-0"
                onClick={() => removeItem(index)}
              >
                <i className="fa-solid fa-trash text-xs text-muted-foreground" />
              </Button>
            </div>
          ))}
        </div>
      )}

      {variables.length < 32 && (
        <Button
          variant="outline"
          size="sm"
          className="h-7 text-xs"
          onClick={addItem}
        >
          <i className="fa-solid fa-plus mr-1 text-xs" />
          {t('aiGateway.providerForm.scriptVariables.add')}
        </Button>
      )}
    </div>
  )
}
