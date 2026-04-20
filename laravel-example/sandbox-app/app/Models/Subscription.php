<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

class Subscription extends Model
{
    protected $table = 'billing_subscriptions';

    protected $connection = 'tenant';

    protected $fillable = [
        'user_id',
        'plan_code',
        'status',
        'unit_amount',
        'currency',
        'seat_count',
        'provider_subscription_id',
        'trial_ends_at',
        'ends_at',
    ];

    protected $casts = [
        'metadata' => 'array',
        'trial_ends_at' => 'datetime',
        'ends_at' => 'datetime',
    ];

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class, 'user_id');
    }

    public function invoices(): HasMany
    {
        return $this->hasMany(Invoice::class, 'subscription_id');
    }

    public function scopeActive($query)
    {
        return $query->whereIn('status', ['trialing', 'active', 'past_due']);
    }
}
