import { useEffect, useState } from "react"
import { check, type Update } from "@tauri-apps/plugin-updater"
import { relaunch } from "@tauri-apps/plugin-process"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"

export function UpdateDialog() {
  const [open, setOpen] = useState(false)
  const [version, setVersion] = useState("")
  const [updating, setUpdating] = useState(false)
  const [update, setUpdate] = useState<Update | null>(null)

  useEffect(() => {
    check()
      .then((u) => {
        if (u?.available) {
          setVersion(u.version)
          setUpdate(u)
          setOpen(true)
        }
      })
      .catch(() => {})
  }, [])

  async function handleUpdate() {
    if (!update) return
    setUpdating(true)
    await update.downloadAndInstall()
    await relaunch()
  }

  return (
    <AlertDialog open={open} onOpenChange={setOpen}>
      <AlertDialogContent size="sm">
        <AlertDialogHeader>
          <AlertDialogTitle>Atualização disponível</AlertDialogTitle>
          <AlertDialogDescription>
            A versão <strong>{version}</strong> está disponível. Deseja atualizar agora?
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={updating}>Agora não</AlertDialogCancel>
          <AlertDialogAction onClick={handleUpdate} disabled={updating}>
            {updating ? "Atualizando..." : "Atualizar"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
